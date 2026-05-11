# 設計判断ログ

書き方:

- 番号は `D` + 4 桁 (`D0001`, `D0002`, ...)。journal からは番号で参照する。
- 状態: `採用` / `却下` / `棄却` / `再考中` / `Superseded by Dxxxx`。
- 既存の判断を見直すときは、新しい番号で「`Dxxxx`: `Dyyyy` を再考」のように追記する形を基本とし、既存の節は状態欄を書き換える程度に留める。

---

## D0001: OpenSBI 経由でブートする

- 日付: 2026-04-29
- 状態: 採用
- 背景: M-mode から自前で bring-up するか、OpenSBI が用意した SBI client として S-mode で起動するかの二択。
- 検討した選択肢:
  - (a) xv6 風: `-bios none` で M-mode 起動、PMP・delegation・タイマトランポリンを自作。
  - (b) OpenSBI: `-bios default` でファームウェアに乗り、S-mode の `0x8020_0000` から始める。
- 採用: (b)。
- 理由:
  - SBI が Console / Timer (`sbi_set_timer`) / HSM (副 hart 起動) / IPI / System Reset を抽象化してくれるので、シェル到達までの工数が小さい。
  - 現代 RISC-V エコシステム (Linux/BSD/Fuchsia/NuttX) と同じ前提に立てる。
  - 学習効果としても「SBI client として書く」経験のほうが応用が利く。
- 影響:
  - 起動時の状態は S-mode、`satp = 0`、`a0 = hartid`、`a1 = DTB` 物理アドレス。
  - M-mode bring-up コードは持たない (将来「外伝」として書く可能性は残す)。
  - タイマは `sbi_set_timer` 越し。Sstc の有無を意識しなくて済む。

## D0002: Sv39 ページングを採用する

- 日付: 2026-04-29
- 状態: 採用
- 背景: RV64 では Sv39 / Sv48 / Sv57 が選べる。
- 採用: Sv39。
- 理由:
  - 3 段で済むので最小工数で立ち上がる。
  - 256 GiB × 2 (上下半分) = 512 GiB は学習用途に十分すぎる。
  - PTE フォーマットは Sv48/Sv57 とほぼ共通なので、後で Sv48 へ拡張する差分は小さい。
- 影響:
  - `satp.MODE = 8`。
  - Canonical 制約: `VA[63:39]` はビット 38 の符号拡張。
  - ページサイズは 4 KiB / 2 MiB / 1 GiB の 3 種。

## D0003: 最初の出力経路は SBI Legacy Console Putchar

- 日付: 2026-04-29
- 状態: **Superseded by D0012**
- 背景: 16550 直叩きと SBI Console のどちらから始めるか。
- 採用: SBI Legacy Console Putchar (EID = `0x01`)。
- 理由:
  - 5 行で書ける。リンカ・ビルド周りの不具合切り分けに集中できる。
  - Console は OS のコアではないので、後で書き直すコストも低い。
- 影響:
  - 当面 `ecall` 経由で 1 文字ずつ送る。バイト列まとめ送りが要れば DBCN 拡張 (EID = `0x4442434E`) に切り替え可。
  - そのうち直 16550 (`0x1000_0000`) に書き直す予定。

## D0004: ターゲットは既存 triple `riscv64gc-unknown-none-elf`

- 日付: 2026-04-29
- 状態: 採用
- 背景: 自前 target JSON を用意するかどうか。
- 採用: 既存 triple `riscv64gc-unknown-none-elf`。
- 理由:
  - `rustup target add` 一発で足りる。
  - `code-model=medany` がデフォルトで都合が良い。
  - 細かい指定が必要になったら `.cargo/config.toml` の `rustflags` で足す。
- 影響:
  - 必要が生じた時点で自前 JSON に切り出す可能性は残す。

## D0005: ツールチェインは nightly

- 日付: 2026-04-29
- 状態: 採用
- 背景: stable で十分か、最初から nightly 固定か。
- 採用: nightly (`rust-toolchain.toml` で固定)。
- 理由:
  - 早晩 `naked_function` / `build-std` / `asm_const` などが要る。
  - 後で切り替える手間を払うより、最初から固定するほうが楽。
- 影響:
  - 不安定機能を意識する必要あり。CI を組むときも toolchain 固定を効かせる。

## D0006: ビルド / 起動は cargo runner

- 日付: 2026-04-29
- 状態: **Superseded by D0007**
- 背景: cargo runner / Makefile / just のどれを玄関にするか。
- 採用: cargo runner (`.cargo/config.toml` の `runner = "qemu-system-riscv64 ..."`)。
- 理由:
  - `cargo run` 一発で QEMU が立ち上がる。最初は手数最小が嬉しい。
  - フラグ違いが増えてきたら Makefile / just に移行する。
- 影響:
  - デバッグ用の `-s -S` 起動など、バリアントが要るときは Makefile か just の追加導入を検討する。

## D0007: ビルド / 起動は Makefile (D0006 を Superseded)

- 日付: 2026-04-29
- 状態: 採用 (D0006 を Superseded)
- 背景: D0006 で cargo runner を採用したが、kernel 開発では debug 起動・gdb 接続・objdump 閲覧など起動の変種が複数欲しくなる。cargo runner は 1 ターゲット 1 つしか書けないので、結局 make / シェル script に逃すことになる。それなら最初から make を玄関にしたほうが素直。
- 検討した選択肢:
  - (a) cargo runner のみ (= D0006)
  - (b) cargo runner + Makefile の hybrid
  - (c) Makefile のみ
- 採用: (c)。
- 理由:
  - 起動の変種を `make run` / `make debug` / `make gdb` / `make objdump` のように一覧で並べられる。
  - xv6 / Linux / NuttX など主要な kernel プロジェクトの流儀と揃う。
  - hybrid (b) は「同じことを 2 通りでできる」ねじれが残る。
- 影響:
  - `.cargo/config.toml` には `runner =` を書かない。default target と `rustflags` のみ。
  - `Makefile` を 1 本追加。最低限のターゲット: `build` / `run` / `debug` / `gdb` / `objdump` / `clean`。
  - 将来「テスト相当」を組むときは Custom Test Framework (`#![feature(custom_test_frameworks)]`) と、`make test` で自前 script を走らせるかを別途決める。

## D0008: メモリレイアウトとデバイスアドレスはハードコード (xv6 流)

- 日付: 2026-04-29
- 状態: 採用
- 背景: ページアロケータ等を組むにあたり、RAM 範囲・MMIO デバイスのアドレスをどう取得するか。
- 検討した選択肢:
  - (a) `fdt` クレートで DTB を読み、RAM 範囲・デバイスアドレスを動的取得
  - (b) DTB を自前パース
  - (c) ハードコード (xv6 流)
- 採用: (c)。`src/memlayout.rs` に `KERNBASE` / `PHYSTOP` / `UART0` / `PLIC` / `CLINT` / `VIRTIO0` などを定数として持つ。
- 理由:
  - QEMU virt のレイアウトは `hw/riscv/virt.c` で事実上固定で、動的取得の必然性が薄い。
  - 「ページアロケータの本筋」と「DTB のデコード」を混ぜるとどちらも学習効果が薄まる。
  - xv6-riscv と同じ流儀になり、参照しやすくなる。
- 影響:
  - QEMU を `-m 128M` 想定でビルド。RAM サイズを変えると再コンパイルが要る。
  - ボード差し替え時には再考。将来 DTB ベースに切り替えるなら別 D で扱う。

## D0009: 物理ページアロケータ以降は「init 1 個先行」で進める (xv6 流の積み上げ)

- 日付: 2026-04-29
- 状態: 採用
- 背景: 物理ページアロケータの次に何を作るか。シェル到達 (短期ゴール) までの組み立て方として、ユーザモード遷移を先に通すか、カーネル内のコンテキストスイッチを先に作るかで道が分かれる。
- 検討した選択肢:
  - (a) init 1 個先行: Sv39 → トラップ → ユーザページテーブル → `sret` で U-mode → ecall (`write`/`exit`) → 埋め込み ELF を init として動かす。スケジューラ・`fork` は後。
  - (b) カーネルスレッド先行: アロケータ → ヒープ → 複数カーネルスレッド + `swtch` + スケジューラ → そのあと U-mode 遷移。
- 採用: (a)。
- 理由:
  - 「Sv39・トラップ・U/S 遷移・syscall」が一直線につながる経験を早く得られる。
  - xv6-riscv の章立てをそのまま参考にしやすい。
  - 1 プロセスのうちは `swtch` もロックも不要で、副作用の少ない状態で個々の機構を検証できる。
- 影響:
  - スケジューラと `fork` は init が動いた後に着手する。
  - trapframe・プロセス構造体は「プロセスごとに 1 個」を最初から前提に置き、後で複数プロセスへスケールできるレイアウトにする (1 プロセス決め打ちで最適化しない)。
  - タイマ割り込み (preemption) はスケジューラを足す段階で本格化する。それまでは「来ても OK」程度のハンドリングで良い。

## D0010: カーネルアドレス空間は identity map (higher-half にしない)

- 日付: 2026-04-29
- 状態: 採用
- 背景: Sv39 有効化時、カーネルを物理と同じ仮想アドレスに置く (identity map) か、上半分 (higher-half、例 `0xffff_ffc0_8020_0000` 起点) に置くかの選択。
- 検討した選択肢:
  - (a) identity map: カーネル仮想 = カーネル物理 (= `0x8020_0000` 起点)。
  - (b) higher-half: カーネルを Sv39 の上半分にリンクし、`satp` 有効化と同時に上半分へ飛ぶ。
- 採用: (a)。
- 理由:
  - `linker.ld` (`. = 0x80200000`) と `_start` をそのまま使える。`satp` 切り替え時に PC を貼り替えるトランポリンが要らない。
  - xv6-riscv と揃うので参考実装をそのまま読める。
  - 学習段階で「仮想化の効果」と「ユーザ/カーネル分離」を 1 ステップに混ぜない。
- 影響:
  - カーネルページテーブルは以下を identity マップで持つ:
    - `[KERNBASE, PHYSTOP)` の RAM (D0008 の定数を使用)
    - `UART0` / `CLINT` / `PLIC` / `VIRTIO0` などの MMIO ページ
  - ユーザ空間とのアドレス衝突はない (Sv39 の下半分 256 GiB のうち、カーネルが使うのは数十 MiB に収まる)。
  - 将来 higher-half に移す価値が出てきた場合は、別 D で「D0010 を再考」として扱う。

## D0011: 最初の init はカーネルにバイナリ埋め込み

- 日付: 2026-04-29
- 状態: 採用
- 背景: FS 未実装の段階で、最初のユーザプロセスをどう持ち込むか。
- 検討した選択肢:
  - (a) `include_bytes!` で init の ELF をカーネル ELF に埋め込み (xv6 の `initcode` に相当)。
  - (b) QEMU の `-initrd` / `-device loader` などで別アーティファクトとして渡す。
  - (c) RAM FS / 簡易 FS を先に作り、そこから読む。
- 採用: (a)。
- 理由:
  - FS が無い段階で動かせる。ビルド成果物がカーネル 1 つに収まる。
  - xv6 の流儀で素直に書ける。
- 影響:
  - init 用のクレート (or サブターゲット) を 1 本立て、ユーザ ELF を生成するビルド経路を Makefile に足す必要が出る。
  - カーネルは埋め込み ELF をパースし、セグメントをユーザページテーブルにマップ、最初の `sret` でエントリへ飛ばす。
  - 将来 FS から exec できるようになったら埋め込み経路は縮退させる (その時点で `D0011` を再考の形で更新する)。

## D0012: Console は最初から 16550 UART 直叩きに統一 (D0003 を Superseded)

- 日付: 2026-04-30
- 状態: 採用 (D0003 を Superseded)
- 背景: D0003 で SBI Legacy Console Putchar を採用したが、(a) モジュール化 → (b) UART 書き直し、と 2 段に分けるより、最初から UART に統一して 1 段にまとめる方が学習段階として素直。SBI Console と UART を並存させるメリットも薄い。
- 検討した選択肢:
  - (a) D0003 のまま SBI Console を保持し、別途 UART 実装を加えてトレイト or enum で抽象化。
  - (b) 型エイリアスで「現行の Console」をコンパイル時切替。
  - (c) SBI Console を撤去し、UART 16550 直叩きの実装 1 本に統一。
- 採用: (c)。
- 理由:
  - 実装が 1 つだけになり、抽象化の議論を持ち越さなくて済む。
  - 実 MMIO デバイスドライバの感覚に早く触れられる。
  - SBI 自体は Timer / IPI / HSM / System Reset で引き続き利用するので、SBI 経由の経験は別経路で得られる。
- 影響:
  - 既存の `SbiConsole` は撤去し、`src/uart.rs` (Uart16550) + `src/console.rs` (println! マクロ + グローバルアクセス) に置き換える。
  - 16550 の最小初期化を行う: `IER = 0` → `LCR = 0x80` (DLAB) → `DLL/DLM` (baud) → `LCR = 0x03` (8N1, DLAB off) → `FCR = 0x07` (FIFO enable + clear)。QEMU 上では baud は無視されるが、実機相当の手順を踏む。
  - OpenSBI のデフォルト PMP 設定では S-mode から `0x1000_0000` に直接アクセスできるので権限上の問題はない。
  - D0003 は `Superseded by D0012` に書き換える。

## D0013: Console 出力は最初から spin::Mutex で保護する

- 日付: 2026-04-30
- 状態: 採用
- 背景: シングルコア・割り込み未実装の現時点では Console を保護する必要は実質ない。だが (c) でトラップが入ると、ロック区間中に割り込みハンドラが `println!` を呼ぶ再入 deadlock の問題に直面する。最初からロックを入れておけば、後でリファクタせずに `push_off`/`pop_off` を学ぶ自然な動機が得られる。
- 検討した選択肢:
  - (a) ZST + lockless で運用し、(c) で Mutex に昇格。
  - (b) `static mut` + `unsafe` で繋ぎ、必要になったら直す。
  - (c) 最初から `spin::Mutex` で保護。
- 採用: (c)。
- 理由:
  - xv6-riscv の流儀と同じ (printf 専用 spinlock + panic 時の lockless 経路)。
  - (c) でトラップを実装するときに `push_off`/`pop_off` (ロック区間中の割り込み禁止) を導入する自然なきっかけになる。
  - panic 経路で「ロックを取らない出力」を持つ必要が出るので、xv6 の `pr.locking` 相当を最初から設計に組み込める。
- 影響:
  - 依存に `spin` crate (default-features = false) を追加。
  - Console は概ね `static CONSOLE: Mutex<Uart16550> = Mutex::new(...)` の形で置き、`println!` は lock を取って書く。
  - panic ハンドラは Mutex を経由しない直叩き経路を持つ (xv6 の `pr.locking = 0` 相当)。
  - 割り込みが入る (c) の段階で `push_off`/`pop_off` (= 同 hart 再入 deadlock 防止) を導入する。

## D0014: トラップ入口は naked function + naked_asm! で書く

- 日付: 2026-04-30
- 状態: 採用
- 背景: trap_entry を `global_asm!` で書くか、`#[unsafe(naked)] extern "C" fn` + `core::arch::naked_asm!` で書くかの選択。`_start` は前者で書いている。
- 検討した選択肢:
  - (a) `global_asm!` + `unsafe extern "C" { fn trap_entry(); }` (= `_start` と同じ流儀)。
  - (b) `#[unsafe(naked)] extern "C" fn trap_entry() -> !` の本体に `naked_asm!` を 1 個。
- 採用: (b)。
- 理由:
  - Rust の関数として名前空間に居るので、`stvec` への登録が `let f: extern "C" fn() -> ! = trap_entry; f as usize` で書ける。関数アイテム → 関数ポインタ → usize の流れが型で読める。
  - 将来 (g) で U→S 経路と分岐させるとき、Rust 側でラッパや属性を取り回しやすい。
  - `naked_functions` は Rust 1.88 で stabilize 済み。feature gate が要らない。
- 影響:
  - 本体は `naked_asm!` を 1 つ呼ぶだけ。普通の `asm!` は使えない。
  - prologue/epilogue は一切付かないので、スタック調整 / `call kerneltrap` / 復帰 / `sret` まで全部 asm 側の責任。
  - `naked_asm!` 冒頭で `.align 2` (= 4-byte 境界) を入れて `stvec` の alignment 要件を担保する。
  - S→S 専用のミニマル版では `struct trapframe` 型を Rust 側で定義しない。xv6 の kernelvec と同様、asm の 256 バイトスタック領域 + C ローカルでの sepc/sstatus 退避という分担で済ませる。U→S 経路を加える (g) の段階で初めてフレーム型を導入する。
  - `_start` を将来 naked function に揃えるかは別途検討 (今回は触らない)。

## D0015: 割り込み禁止連動の Spinlock を自前実装する

- 日付: 2026-05-01
- 状態: 採用
- 背景: 割り込みハンドラから `println!` を安全に呼ぶには、Console の Mutex が「ロック取得時に割り込み禁止 / 解放時に元に戻す」連動をしている必要がある。`spin` crate の `Mutex` にこの仕組みはない。xv6 の `acquire` / `release` は `push_off` / `pop_off` を内部で呼ぶ作りになっている。
- 検討した選択肢:
  - (a) `spin::Mutex` をラップする `IrqSafeMutex<T>` を作る (lock 前後で push_off/pop_off)。
  - (b) `lock_api` クレートの `RawMutex` を実装する独自型。
  - (c) `Spinlock<T>` を `AtomicBool` + `UnsafeCell<T>` で自前実装し、xv6 の `spinlock.c` に倣う。
- 採用: (c)。
- 理由:
  - 学習目的。Mutex の実装そのものを書く経験を得たい。
  - xv6-riscv の `spinlock.c` (50 行程度) を Rust に翻訳する規模で、見通しが良い。
  - `spin` crate を依存から外せて構成が単純になる。
- 影響:
  - `src/cpu.rs` (`Cpu { noff, intena }` + `push_off` / `pop_off` 等) と `src/spinlock.rs` (`Spinlock<T>` + `SpinlockGuard`) を新規追加。
  - `Cargo.toml` から `spin` crate を削除。
  - `Console` を `Spinlock<Uart16550>` に置き換え。
  - xv6 の self-deadlock check (`holding(lk)`) は当面省略。再帰取得は無限 spin。必要なら後で追加。
  - シングルコア前提で `Cpu` は `static mut` 1 個。SMP 化 (D0009 で後回しと決定済み) のときに hartid 配列化が必要。

## D0016: 物理ページアロケータは xv6 風 freelist

- 日付: 2026-05-01
- 状態: 採用
- 背景: シェル到達までの利用シーン (ページテーブル用、ユーザプロセス本体、将来のヒープのバッキング) は 4 KiB 1 枚単位の確保で足りる。
- 検討した選択肢:
  - (a) Freelist (xv6 流): 空きページ先頭 8 バイトに次ポインタを書き、stack push/pop で alloc/free。メタ領域不要、実装 50 行未満。
  - (b) Bitmap: ページ数ぶんビット列。連続確保が要るときに有利。
  - (c) Buddy: DMA バッファ / 巨大ページの世界。今はオーバキル。
- 採用: (a)。
- 理由:
  - シェル到達まで連続確保の要求は出ない見込み。
  - xv6 と同じ流儀で実装の参照が読みやすい。
  - 実装が小さく、上位レイヤへの心理的負荷が低い。
- 影響:
  - 空きページ内に `Run { next: *mut Run }` を埋め込むので **identity map 必須** (D0010 と整合)。
  - 連続ページ確保 (DMA、巨大ページ) が必要になったら別 D で再考。
  - `kfree` で `0x05` 埋めして use-after-free を炙り出す。`kalloc` は zero fill しない (呼び側責任)。

## D0017: アドレス型 `PhysAddr` / `VirtAddr` を newtype で導入

- 日付: 2026-05-01
- 状態: 採用
- 背景: ページアロケータ + Sv39 ページテーブル構築で物理/仮想を取り違える事故を型で防ぎたい。
- 検討した選択肢:
  - (a) `usize` で回す (xv6 流)。
  - (b) newtype を最初から導入。
  - (c) 今は `usize`、(f) で導入。
- 採用: (b)。**ただし `KERNBASE` / `PHYSTOP` / MMIO 系の既存定数は `usize` のまま残す**。
- 理由:
  - Sv39 で必ず要る型なので先取りしたい。
  - MMIO ベースは「物理 RAM のページ」とは性質が違い、`PhysAddr` を被せると概念が薄まる。MMIO がどっち側で扱われるかは Sv39 を入れてから決めるほうが早い。
- 影響:
  - `src/memlayout.rs` に配置。`PhysAddr` には `as_usize` / `is_page_aligned` / `page_round_down` / `page_round_up` / `as_mut_ptr<T>` を実装、レシーバは `Copy` 型の慣習に揃えて `self`。
  - `VirtAddr` は呼び側が居ないので定義のみ。(f) で本格利用。
  - `kernel_end()` の戻り値は `usize` のまま (呼び側で `PhysAddr` ラップ)。

## D0018: グローバル割り込み有効化は kmain に集約

- 日付: 2026-05-01
- 状態: 採用
- 背景: `timer::init` が `sstatus.SIE = 1` を中で行っていたため、関数名からは読めないグローバル副作用になっていた。`plic::init` を先に呼んでも順序が偶然嵌って動いていた。
- 検討した選択肢:
  - (a) 現状維持 (`timer::init` がグローバル enable も担当)。
  - (b) `plic::init` 側でも担当する (二重保険)。
  - (c) 各サブシステムは自分の `sie` ビットだけ触り、`sstatus.SIE` は `kmain` 末尾で `cpu::intr_on()` を 1 回だけ呼ぶ (xv6 流)。
- 採用: (c)。
- 理由:
  - 「個別 enable (sie.STIE / sie.SEIE)」と「グローバル enable (sstatus.SIE)」は責務が違うので場所で分ける。
  - 順序依存が消える。
  - xv6 `main.c` 末尾の `intr_on(); scheduler();` と揃う。
- 影響:
  - `cpu::intr_on` を `pub` に公開。
  - `timer::init` から `sstatus.SIE = 1` を削除。
  - `kmain` 末尾で `cpu::intr_on()`。
  - 各 init は SIE off で走るため、ロック取得時の `push_off` も intena=false を保存→復帰するだけ。

## D0019: W^X 権限分離をカーネル identity map で実装

- 日付: 2026-05-01
- 状態: 採用
- 背景: (f) でカーネル identity map を張るにあたり、`[KERNBASE, PHYSTOP)` 全体を一括 RWX で張るか、text / rodata / data を分けるかの判断が必要。
- 検討した選択肢:
  - (a) 一括 RWX (xv6 の最初期版に近い形)。
  - (b) text=RX、rodata=R、data+bss+free=RW で分離 (W^X)。
- 採用: (b)。
- 理由:
  - `linker.ld` に `__etext` / `__erodata` の 4 KiB align 境界を 1 行ずつ足すだけで実装でき、コスト対効果が高い。
  - 後で (g) の U-mode 遷移を入れる前から、カーネル側のメモリ保護を「正しい」状態にしておけるとデバッグが楽。
  - 学習目的としても W^X を最初から踏むほうが教育的。
- 影響:
  - `linker.ld` に `.text` / `.rodata` の終端で `ALIGN(4096); __etext = .;` / `__erodata = .;` を追加。
  - `vm::kvmmake` で 3 区間に分けて `kvmmap_range` を呼ぶ (`[KERNBASE, __etext)` = R|X、`[__etext, __erodata)` = R、`[__erodata, PHYSTOP)` = R|W)。
  - MMIO は X 不要なので `R | W` のみ。U bit はカーネルマップでは立てない。
  - 各境界で最大 4 KiB - 1 のパディングが発生 (合計 12 KiB 以下、誤差)。

## D0020: PTE の A / D bit を `Pte::new_leaf` で強制 OR

- 日付: 2026-05-01
- 状態: 採用
- 背景: RISC-V Privileged §4.4.1 で A / D bit の更新方式は 2 通り (Svade = OS が立てる、Svadu = HW が atomic に立てる)。退避・writeback・COW のいずれも実装しない学習段階では、A / D を OS 側でどう扱うかの設計が必要。
- 検討した選択肢:
  - (a) xv6 流: 呼び側責任 (`flags` に含めずに渡す)。QEMU (Svadu) では fault しないが、Svade 実機では `A=0` のページにアクセスすると page fault。
  - (b) `Pte::new_leaf` 内で `| PTE_A | PTE_D` を強制 OR。Svade / Svadu いずれでも A/D 起因の fault が原理的に発生しない。
  - (c) `kvmmake` 側で flags に毎回明示する。
- 採用: (b)。
- 理由:
  - 退避を実装しない以上、A / D 情報を OS が観測する場面が無い。「常時 1」でも情報量の損失なし。
  - `kvminithart` 直後の page fault デバッグで A/D 起因の可能性を排除できる。
  - 実機 (Svade) への可搬性が無料で手に入る。
- 影響:
  - `Pte::new_leaf` は `| PTE_V | PTE_A | PTE_D` を強制。
  - 中間 PTE (`Pte::new_table`) には A/D を立てない (CPU は中間 PTE の A/D を見ないため意味なし)。
  - 将来 page replacement / writeback / COW を実装する段階で `new_leaf` の強制 OR を外して fault ハンドラ経由の更新に切り替える必要がある (その時点で本 D を再考)。

## D0021: CSR アクセサは cpu.rs に集約

- 日付: 2026-05-01
- 状態: 採用
- 背景: (f) で `satp` / `sfence.vma` のラッパが必要になった。既存の `sstatus.SIE` 系 (`intr_get` / `intr_off` / `intr_on`) は `cpu.rs` にあるが、`vm.rs` に追加する選択肢もある。
- 検討した選択肢:
  - (a) `vm.rs` に `r_satp` / `w_satp` / `sfence_vma` を置く (use site と同居)。
  - (b) `cpu.rs` に集約 (xv6 の `riscv.h` 流)。
  - (c) `riscv.rs` を新設して CSR 専用モジュール化。
- 採用: (b)。
- 理由:
  - xv6-riscv の `riscv.h` と同じ流儀で、CSR 操作の所在地が 1 箇所に集まる。
  - すでに `sstatus` 系が cpu にあるので、追加先として最も自然。
  - (c) は将来 CSR が増えてから検討すればよい (今は前倒し過ぎ)。
- 影響:
  - `cpu.rs` に `r_satp` / `w_satp` / `sfence_vma` を `unsafe fn` で追加。
  - `vm::kvminithart` から呼ぶ。
  - 今後 `mstatus` 等の他 CSR が要るときも cpu.rs に追加する流儀。

## D0022: user PT は xv6 流の raw 関数群で扱う

- 日付: 2026-05-02
- 状態: 採用
- 背景: user PT を Rust の所有型 (`UserPagetable` newtype + `Drop`) で表現するか、xv6 の `pagetable_t` 流に raw 関数群で扱うかの選択。kernel PT (`kvmmake -> &'static mut PageTable`) は唯一・永続なので妥当だが、user PT は複数生成・解放されるため別の流儀が必要。
- 検討した選択肢:
  - (a) `UserPagetable` newtype + `Drop` で `uvmfree` 自動化。
  - (b) `uvmcreate -> *mut PageTable` 等の自由関数。所有・解放は呼び出し側 (将来の `Process` 構造体) が持つ。
- 採用: (b)。
- 理由:
  - xv6 の `pagetable_t` と意味が一致するので参考実装が直接読める。
  - 後で `uvmfree` 経路を追加するときにそのまま乗る (戻り値が `&'static mut` だと前提が崩れる)。
  - `Process` 構造体が PT を所有する形にスケールしやすい。
- 影響:
  - kernel PT は `kvmmake -> &'static mut PageTable` のまま据え置き (本当に唯一・永続)。
  - user PT の所有モデル (`size` 範囲 / trampoline 配置 / 解放経路) は (g-2)・(g-3) の段階で D0023 以降に決定。
  - `mappages` / `walk` は引き続き `&mut PageTable` を取り、user 側からは `unsafe { &mut *pt }` で渡す。

## D0023: U/S 切替はトランポリン方式 (xv6 流)

- 日付: 2026-05-02
- 状態: 採用
- 背景: U-mode と S-mode で `satp` を切り替える瞬間、その前後の数命令が新旧両方の PT で **同一 VA に見えていなければならない**。`csrw satp` の直後の命令フェッチで PC が新しい PT で walk されるため、PC の指す VA が新 PT に存在しないと即 fault する。実現方式に複数の流儀がある。
- 検討した選択肢:
  - (a) xv6 流: 同一物理ページを kernel PT と user PT の `MAXVA - PGSIZE` にマップし、その上に trap 出入口の asm を置く。
  - (b) user PT にもカーネル領域を identity map で同居させ (PTE_U=0)、トランポリン不要。
  - (c) S-mode でも user satp のまま走り、必要な kernel 領域を user PT 側から見えるようにする。
- 採用: (a)。
- 理由:
  - kernel / user の PT を完全分離できるので、KPTI 相当の隔離が最初から成立する。
  - xv6-riscv の `trampoline.S` / `trap.c` を参考にしやすい。
  - 学習目的としても「同じ VA で satp が切り替わる」体験を踏むほうが教育的。
  - (b) は user PT にカーネル領域がそのまま見えてしまうので隔離が緩い。(c) は S-mode のスタック・グローバルを user PT に晒す前提になり (b) と同根。
- 影響:
  - `memlayout.rs` に `MAXVA = 1 << 38` / `TRAMPOLINE = MAXVA - PGSIZE` / `TRAPFRAME = MAXVA - 2 * PGSIZE` を追加。
  - `MAXVA` を `1 << 39` ではなく `1 << 38` に丸めるのは Sv39 の sign extension 境界 (`VA[63:39]` のチェック) を踏まないため (xv6 も同じ)。
  - `linker.ld` に `.text.trampoline` セクションを切り、`__trampoline_start` / `__trampoline_end` を 4 KiB 境界で export。
  - kernel PT・user PT 両方に `TRAMPOLINE` で同一物理ページをマップ (RX、PTE_U=0)。
  - `stvec` は走行 mode で切り替える: S-mode 中は `kernelvec` (= 既存 `trap_entry`)、U-mode 中は `TRAMPOLINE + (uservec offset)`。`usertrapret` で sret 直前に切り替え、`usertrap` 冒頭で `kernelvec` に戻す。
  - `usertrap` と `kerneltrap` は完全分離。dispatch は `stvec` の値で決まり、`sstatus.SPP` で振り分けない。
  - trapframe は user PT に **借用マッピング** (PTE_U=0) で `TRAPFRAME` に貼る。`uservec` は `satp = user` で走るため、kernel 側のデータには直接アクセスできず、`kernel_satp` / `kernel_sp` / `kernel_trap` / `kernel_hartid` を `usertrapret` であらかじめ trapframe に書き込んでおく必要がある (これは SMP のためではなく satp 境界そのものに起因する)。

## D0024: 最小 Process 構造体を (g-2-c) で導入

- 日付: 2026-05-02
- 状態: 採用
- 背景: trapframe ページを所有する場所が必要。D0009 で「プロセスごとに 1 個の前提に置く」と決めているので、ここで `Process` 構造体を導入するのが自然。
- 検討した選択肢:
  - (a) `static TRAPFRAME` / `static USER_PT` を直接置く (= シングルプロセス決め打ち)。
  - (b) `Process` 構造体を新設し、`pagetable` / `trapframe` / `sz` を集める。
  - (c) 各リソースを別々のグローバルに持って後で寄せ集める。
- 採用: (b)。
- 理由:
  - D0009 / D0022 の方針と整合し、複数プロセスへの拡張時も同じ構造を再利用できる。
  - trapframe の所有 (kalloc → 解放時 `kfree`) を Process に集約できる。
- 影響:
  - `src/proc.rs` を新設。当面は `Process { pagetable: *mut PageTable, trapframe: *mut Trapframe, sz: usize }` のみ。
  - `state` / `pid` / `name` / `kstack` / `lock` / `context` は trap 経路 (g-3) とスケジューラ着手時に追加。
  - 当面はインスタンス 1 個で運用するが、`static PROC: Process` のような「型レベルで 1 個固定」の置き方はせず、生成は関数で行う (= 後で `[Process; NPROC]` に拡張できるよう、シングルプロセス前提を残さない)。

## D0025: kstack は kernel PT の識別マップ上に置く (xv6 の高位 VA + ガードページは採用しない)

- 日付: 2026-05-02
- 状態: 採用
- 背景: xv6 は各 proc の kstack を kernel PT の `MAXVA - hartid * 2 * PGSIZE - PGSIZE` 等の固定高位 VA に貼り、すぐ下に未マップのガードページを置いてスタックオーバフローを page fault で検出する。我々はシングルコア・1 プロセスから始まり、kalloc が `[__kernel_end, PHYSTOP)` を識別マップに乗せているので、kstack も識別マップそのまま使える余地がある。
- 検討した選択肢:
  - (a) xv6 流: 高位 VA + ガードページ。スタックオーバフローを page fault で検出可能。
  - (b) 識別マップに乗せる、ガードページ無し。
- 採用: (b)。
- 理由:
  - シングルコア・1 プロセス段階では複雑さに見合うリターンが薄い。
  - `Process::new` 内で `let kstack_pa = kalloc()?; let kstack = kstack_pa.as_usize();` だけで完結し、`kvmmake` への手出し不要。
  - スタックオーバフローの検出は当面犠牲にする。深い再帰や巨大ローカル変数を持ち込まない運用で当面は問題ない見込み。
- 影響:
  - `Process { ..., kstack: usize }` で `kstack` は kalloc 由来の PA そのもの (= xv6 と同じく "底" を保存)。`kernel_sp = kstack + PGSIZE` は使う側で計算。
  - スタックオーバフローはサイレントなメモリ破壊になりうる (= デバッグ時に痛い目を見る可能性は残す)。
  - SMP 化 / 複数プロセス化または fork 実装のどこかで、ガードページ導入を再考する。その時点で本 D を再考する形で更新する。

## D0026: ユーザプログラムは multi-bin の単一 Rust crate に集約する

- 日付: 2026-05-02
- 状態: 採用
- 背景: (i) で init を本物の ELF として埋め込むにあたり、ユーザプログラムをどういう単位で管理するかを決める必要がある。シェル到達までに init / sh / ls / cat / echo 程度の数本に増える見込みで、それぞれを別 crate にすると Cargo.toml だらけになる。
- 検討した選択肢:
  - (a) `user/` ディレクトリに 1 個だけ Cargo crate を置き、`src/bin/<name>.rs` で各プログラムを別バイナリ化。共有ライブラリ (`_start` / `panic_handler` / syscall ABI スタブ) は同 crate の `src/lib.rs`。
  - (b) C で書く (xv6 流)。`riscv64-unknown-elf-gcc` を Makefile から呼ぶ。
  - (c) `rustc` 単体 + Makefile (Cargo 抜き)。
  - (d) アセンブリ直書き (init のみ可、シェル以降は不可)。
- 採用: (a)。
- 理由:
  - Cargo.toml がプログラム数に依らず 1 個で済む。
  - `src/lib.rs` の ulib 相当 (= `_start` / panic / syscall スタブ) を全プログラムで自然に共有できる。
  - Rust の no_std user 空間がどう構成されるかを学べる (kernel 側だけでは見えない領域)。
  - C を採れば xv6 のソースをほぼコピペできる利点はあるが、別 toolchain 管理と「Rust 一貫」の方針が崩れるコストの方が大きい。
- 影響:
  - `user/` ディレクトリを新設。当初は `user/Cargo.toml` + `user/src/lib.rs` + `user/src/bin/init.rs` の 3 ファイル。
  - kernel と user の syscall ABI は両方 Rust なので、共有定数 (syscall 番号など) を `kernel/` 側にも `user/` 側にも置くか、別 crate (workspace 内の `common` 的なもの) に切り出すかは (h) 着手時に改めて判断する。
  - Makefile に user 側のビルドターゲット (`cargo build --manifest-path user/Cargo.toml --bin init`) を追加。
  - kernel 側の `include_bytes!` が取り込むのは `user/target/.../init` の ELF。パスの固定方法 (Makefile 側で対応するか `build.rs` で吸収するか) は (i) 着手時に決定する。
  - 新しいプログラムを足す場合は `user/src/bin/<name>.rs` を増やすだけ。Cargo.toml は触らない (Cargo の `[[bin]]` 自動生成規約に乗る)。

## D0027: syscall ABI は Linux 番号 + Linux errno + POSIX semantics

- 日付: 2026-05-03
- 状態: Superseded by D0031
- 背景: (h) で syscall ABI の雛形を入れるにあたり、番号体系・戻り値規約・errno をどう揃えるかの判断が必要。シェル到達後にこの OS 向けの libc (musl の port 等) を被せる構想がある。
- 検討した選択肢:
  - (α) Linux RISC-V generic 番号 (`__NR_write = 64`, `__NR_exit = 93`, ...) + Linux errno + POSIX API semantics。
  - (β) xv6 流の独自連番 (`SYS_fork = 1`, ..., `SYS_write = 16`) + errno なし (`-1` 一択) + POSIX 縮小版 semantics。
  - (γ) 折衷: API semantics は POSIX、番号は独自連番、errno は Linux 番号。
- 採用: (α)。
- 理由:
  - 番号も errno も理屈上は libc の中で変換できるが、無料で Linux に揃えられるなら変換テーブルを恒久的に持たずに済む。
  - xv6 流連番だと、(i) 以降に `write` を実装した瞬間 `SYS_write` の番号が Linux と乖離し、libc 被せ時に番号変換層が必須になる。
  - errno を持たない xv6 流は libc が「失敗理由」を取り出せず、`-1` を全部 `errno = EIO` のように丸める歪みが出る。
  - 学習目的としても、現代 Unix-like の標準寄り (= Linux generic syscall 表) を踏むほうが応用が利く。
- 影響:
  - `src/syscall.rs` に番号定数を Linux 名で置く (例: `pub const SYS_EXIT: usize = 93;` `pub const SYS_WRITE: usize = 64;`)。
  - 戻り値規約: `a0` に `i64`、失敗は `-errno` (Linux と同じ番号、例: `EBADF = 9`, `EFAULT = 14`, `ENOSYS = 38`)。errno 定数は失敗 syscall を実装する都度 1 つずつ追加し、最初は空でよい。
  - API semantics は POSIX (= `ssize_t write(int fd, const void *buf, size_t count)` 等)。Linux 拡張 (`exit_group`, `clone`, `openat` の `dirfd` 等) は当面採らず、必要になった時点で別途判断。
  - 学習用便宜 syscall (Linux にも POSIX にも無いもの、例: `putc(ch)`) は **Linux 予約域から外れた高番号** (`1024+`) に置く。`putc` は (i) で `write` が動いたら撤去予定。
  - 番号定数を kernel と user で共有する方法 (D0026 で「(h) 着手時に判断」と保留したもの) は引き続き保留。(h) では INITCODE が `global_asm!` 内に数値直書きなので、共有機構なしで済む。user crate を立ち上げる (i) のタイミングで `common` crate を切るかどうかと一緒に決める。
  - 将来 Linux と非互換にする選択 (例: `fork` を残さず `clone` のみにする) を採る場合は別 D で「D0027 を再考」として扱う。

## D0028: kernel エラーはモジュール内ローカル enum、syscall 境界で errno に変換

- 日付: 2026-05-04
- 状態: Superseded by D0031
- 背景: (i-4) で `copyin` の戻り値型を決めるにあたり、kernel 内エラーの表現方法の方針を決める必要が出た。(i) 以降に FS / block / VM のエラー型が増えてくるので、最初の方針を立てておかないと毎回判断が再発する。
- 検討した選択肢:
  - (A) 単一 `KernelError` enum (フラット)。Linux for Rust 流。すべての層が同じ enum に variant を足し、syscall 境界に巨大 `match` を 1 個。
  - (B) モジュールごとに固有 enum (`CopyError` / `FsError` / `BlockError` ...) + `From` で階層化。下層エラーを上層が wrap する。`?` で連鎖する。
  - (C) 二層: モジュール内は固有 enum、syscall 境界で共通の `Errno` newtype (`#[repr(transparent)] struct Errno(i64)`) に変換。`From<CopyError> for Errno` を境界で実装。
- 採用: 当面 (B) の精神 (= モジュール内固有 enum) を採り、`syscall.rs` 内に `errno_of_xxx(XxxError) -> i64` を per-module で置く。共通 `Errno` 型は導入しない。
- 理由:
  - (i-4) 時点ではエラー型が `CopyError` 1 つしかなく、`Errno` newtype を導入してもメリット (= From impl の統一) が見えない。先取り抽象になる。
  - (A) は Rust で書く意味が薄い。errno enum と本質的に同型で、モジュール内ローカルなエラー (= 「FS の中でしか起きない」) も全部グローバルに見える。網羅性チェックの効きが悪くなる。
  - (B) は各モジュールが「自分が返しうるエラー」を型で表現できて enum の網羅 match が効く。下層 variant を上層 enum が wrap するコストは、層が深くなったときに `From` の連鎖で自然に吸収される。
  - エラーの種類が 3〜4 個に増えた段階で (C) への昇格 (= 共通 `Errno` 型導入) を検討する。具体的には FS / block 層が登場した時点で本 D を再考する。
- 影響:
  - `src/vm.rs` に `pub enum CopyError { Fault }` を置く。
  - `src/syscall.rs` に `EBADF: i64 = 9` / `EFAULT: i64 = 14` 定数 + `errno_of_copy(CopyError) -> i64` を置く (戻り値は `-errno` を返すため呼び出し側で符号反転する)。
  - kernel 内ロジックは errno 数字を一切知らない (= `CopyError::Fault` で語る)。errno への変換は syscall 境界 (= `errno_of_copy`) という単一の場所でのみ発生する。
  - FS / block 層が登場した時点で本 D を再考し、共通 `Errno` 型 + `From` impl 統一に進むか、per-module `errno_of_xxx` のままで通すかを判断する。再考時は新番号で「`D0028` を再考」として記録する。
  - 関連: D0027 で errno 番号体系を Linux に揃えると決めているので、`errno_of_xxx` の戻り値定数は Linux generic 番号を踏襲する。

## D0029: 初期 scheduler は xv6 型 per-process lock + raw `swtch` で実装する

- 日付: 2026-05-05
- 状態: 採用
- 背景: (j) で scheduler / context switch を導入するにあたり、process table・排他・`swtch` をまたぐ lock の扱いを決める必要があった。当初は lock なし + シングルコア + interrupt off で進める案も検討したが、`yield` / `sched` を入れる段階でかえって割り込み制御と状態遷移の整合が複雑になった。
- 検討した選択肢:
  - (a) `static mut PROCS` + lock なし。process table 操作中は `intr_off` / `push_off` で単一コア排他を代用する。
  - (b) `Spinlock<T>` の RAII guard を process lock として使う。
  - (c) xv6 型: per-process lock を明示 `acquire` / `release` する raw lock として持ち、scheduler / `sched` / `yield` / `exit` では lock を持ったまま `swtch` する。
- 採用: (c)。
- 理由:
  - `p.state` / `p.context` / kernel stack ownership / `cpu.proc` は一体の不変条件を持つので、state 更新だけを lock する設計では不十分。
  - `swtch` をまたいで lock を保持する xv6 型のほうが、scheduler context と process kernel context の所有関係を素直に表せる。
  - Rust の RAII guard は「acquire した context と release する context が異なる」scheduler 経路と相性が悪い。ここは low-level kernel code として raw API に寄せるほうが正直。
  - `RawSpinlock` の owner は `AtomicUsize` の CPU id で持つ。xv6 の `struct cpu *` owner と同じ目的 (= `holding` / misuse detection) だが、Rust の共有メモリモデルに乗せやすい。
- 影響:
  - `Process` に `lock: RawSpinlock` / `state` / `pid` / `context` を追加。
  - `Context` は `#[repr(C)]` で `ra`, `sp`, `s0..s11` を保存する。`src/asm/swtch.S` の offset と手同期する。
  - scheduler は `p.lock.acquire()` → `Runnable` なら `Running` / `cpu.proc = p` → `swtch(cpu.context, p.context)` → 復帰後 `cpu.proc = null` → `p.lock.release()` の形。
  - 初回 process への `swtch` では `forkret()` が `p.lock.release()` してから `usertrapret()` へ入る。
  - `yield_cpu()` / `exit()` は `p.lock.acquire()` → state 遷移 → `sched()`。scheduler に戻った後の release は scheduler 側が行う。
  - `sched()` は `p.lock.holding()` / `state != Running` / interrupt disabled / `cpu.noff == 1` を assert し、`swtch` 前後で `cpu.intena` を保存・復元する。
  - U-mode に戻る `usertrapret()` 冒頭で `cpu.noff == 0` を assert し、critical section を保持したまま user へ戻るバグを検出する。

## D0030: 初期 preemption は supervisor timer interrupt + 100ms time slice で行う

- 日付: 2026-05-05
- 状態: 採用
- 背景: scheduler / `yield_cpu` が入ったため、timer interrupt から process を preempt して round-robin 動作を確認する段階に進んだ。U-mode 実行中の timer interrupt は `kerneltrap` ではなく trampoline 経由で `usertrap` に入るため、trap 経路の扱いを明確にする必要があった。
- 検討した選択肢:
  - (a) timer preemption は後回しにし、`exit` / 手動 `yield` のみで scheduler を確認する。
  - (b) timer interrupt を `kerneltrap` 側だけで扱う。
  - (c) `kerneltrap` / `usertrap` の両方で interrupt code 5 を扱い、process 実行中 (`cpu.proc != null`) の timer で `yield_cpu()` する。
- 採用: (c)。
- 理由:
  - U-mode 中は `stvec = uservec` なので、timer interrupt は `usertrap` に到達する。preemption を実現するには `usertrap` 側の interrupt handling が必須。
  - scheduler context では `cpu.proc == null` なので、timer handler は `cpu.proc != null` のときだけ `yield_cpu()` することで scheduler 自身の再スケジュールを避けられる。
  - 100ms (`mtime` 10MHz で `INTERVAL = 1_000_000`) は初期デバッグで preemption を観測しやすく、1s より短く、10ms よりログが暴れにくい。
- 影響:
  - `timer::handle()` は先に `schedule_next()` し、その後 `proc::myproc() != null` なら `proc::yield_cpu()` を呼ぶ。
  - `usertrap()` は `scause` の interrupt bit と code を分解し、interrupt code 5 (= supervisor timer interrupt) / 9 (= supervisor external interrupt) を処理する。
  - `kerneltrap()` は S-mode 実行中の timer / external interrupt 用として残る。
  - 現在の `init` は preemption 観測用に busy loop と出力を含む。通常の init 形態に戻すタイミングは fork/exec 実装前に再確認する。

## D0031: D0027 / D0028 を再考し、初期 syscall ABI は xv6 に揃える

- 日付: 2026-05-05
- 状態: 採用
- 背景: `fork` を実装する段階で、Linux RISC-V には素朴な `fork` syscall がなく、`clone` 系の ABI を背負う必要があることが問題になった。学習プロジェクトとしては xv6 の構造を追うほうが今の到達点に合うため、D0027 の Linux generic 番号 + errno 方針と、D0028 の errno 変換方針を再考した。
- 検討した選択肢:
  - (a) D0027 のまま Linux generic 番号 + Linux errno + POSIX semantics を維持する。
  - (b) syscall 番号だけ xv6 に寄せ、エラーは Linux/POSIX の `-errno` を維持する。
  - (c) syscall 番号・失敗戻り値とも xv6 に寄せる。
- 採用: (c)。
- 理由:
  - 現段階の主目的は Linux 互換 ABI ではなく、xv6 型の process / fork / wait / exec / shell の流れを理解すること。
  - Linux RISC-V の `clone` ABI は flags / child stack / TLS / ptid / ctid など、今扱いたい学習対象より広い概念を要求する。
  - xv6 の syscall 番号 (`fork = 1`, `exit = 2`, `write = 16` など) に揃えると、以後の xv6-riscv 参照が素直になる。
  - 失敗を `-1` に丸めることで、kernel 内の初期エラー表現を単純に保てる。詳細な errno は FS や libc を考える段階で再検討すればよい。
- 影響:
  - kernel / user の syscall 番号を xv6 風に変更する: `SYS_FORK = 1`, `SYS_EXIT = 2`, `SYS_WAIT = 3`, `SYS_READ = 5`, `SYS_WRITE = 16`。
  - syscall 失敗は原則 `-1` (`SYSERR`) に丸める。`EBADF` / `EFAULT` / `EINVAL` / `ENOSYS` と `CopyError` から errno への変換は撤去する。
  - `copyin` は失敗理由を区別せず `Option<()>` を返す。
  - D0027 と D0028 は履歴として残し、状態を `Superseded by D0031` にする。

## D0032: user synchronous exception は kernel panic ではなく process kill として扱う

- 日付: 2026-05-07
- 状態: 採用
- 背景: user program が不正な address にアクセスした場合、kernel 全体を panic させるのではなく、その process だけを終了させる必要が出た。今後 shell から不正な program や壊れた pointer を渡す状況が増えるため、user fault は OS 全体の失敗として扱わない方針を明確にした。
- 検討した選択肢:
  - (a) 従来通り panic する。
  - (b) fault 情報をログに出して該当 process を `exit(-1)` させる。
  - (c) signal 的な機構を導入する。
- 採用: (b)。
- 理由:
  - user process の不正動作は kernel bug ではない。kernel は faulting process を終了させ、親が `wait` で失敗を観測できるようにするのが自然。
  - (c) は signal delivery / user handler / blocked signal など、現段階には重すぎる。
  - `scause` / `sepc` / `stval` を出すことで、学習・デバッグ上必要な情報は残せる。
- 影響:
  - `cpu.rs` に `stval` read helper を追加。
  - `usertrap()` は syscall 以外の synchronous exception を `usertrap: killing pid ...` としてログ出力し、`proc::exit(-1)` に流す。
  - kernel mode trap は引き続き kernel bug として panic 対象にする。

## D0033: syscall の戻り値処理は `Return` と `Replaced` を区別する

- 日付: 2026-05-07
- 状態: 採用
- 背景: `exec` 成功時は呼び出し元 program に戻らず、trap return 先を新しい program の entry に置き換える。一方、従来の syscall 共通処理は全 syscall の最後に `trapframe.a0 = retval` を行っていたため、`proc::exec` が `_start(argc, argv)` 用に設定した `a0 = argc` を上書きしてしまう。
- 検討した選択肢:
  - (a) `SYS_EXEC` 成功時だけ special-case して `a0` を上書きしない。
  - (b) syscall handler の戻り値を `SyscallResult::{Return(i64), Replaced}` にする。
  - (c) `sys_exec` 成功時に直接 user return まで行い、呼び出し元へ戻らない関数にする。
- 採用: (b)。
- 理由:
  - `exec` の本質は「return value を返す syscall」ではなく「user context を置き換える syscall」なので、型で区別した方が読みやすい。
  - (a) は小さいが、syscall 共通層に `SYS_EXEC && ret == 0` のような暗黙契約が残る。
  - (c) は trap return 経路が `exec` だけ分岐し、trampoline / `usertrapret` の責務が分かりにくくなる。
- 影響:
  - 通常 syscall は `SyscallResult::Return(ret)` を返し、共通処理が `a0` に書き戻す。
  - `sys_exec` 成功時は `SyscallResult::Replaced` を返し、共通処理は `a0` を触らない。
  - `sys_exec` 失敗時は旧 address space に戻るので、通常通り `Return(-1)` として user に失敗を返す。

## D0034: `exec` argv は C 風の thin pointer 配列 + NULL 終端として渡す

- 日付: 2026-05-07
- 状態: 採用
- 背景: 簡易 shell に進む前に `exec(path, argv)` を持たせる必要が出た。Rust の `&[&[u8]]` は user/kernel 境界を越える ABI としては fat pointer 配列になり、kernel が期待する `char **` 形式と一致しない。
- 検討した選択肢:
  - (a) user syscall ABI として `&[&[u8]]` 相当の Rust slice-of-slice を渡す。
  - (b) C の `execv` と同じく、`argv` は `*const *const u8` の thin pointer 配列 + NULL 終端にする。
  - (c) user が `argc` と `argv` pointer array を別々に渡す。
- 採用: (b)。
- 理由:
  - kernel は user memory から pointer array を 1 word ずつ `copyin` し、各 string を `copyinstr` するだけでよい。
  - C / xv6 の `exec` と同じ形なので、今後 shell 実装や参考実装との対応が取りやすい。
  - Rust の `&[&[u8]]` は `(ptr, len)` の fat pointer を含み、安定した syscall ABI として扱いづらい。
  - `argc` は kernel が NULL 終端を走査して決められるため、user から別途渡す必要はない。
- 影響:
  - user 側 wrapper は `execv_cstr(path: &[u8], argv: &[*const u8])` とする。
  - `path` と各 `argv[i]` は NUL 終端済みである必要がある。
  - `argv` 自体は NULL pointer で終端する。`argv == NULL` および空 argv は当面失敗扱いにする。
  - kernel 側は `copy_argv` で旧 address space から `KernelArgs` へコピーし、成功時 `argc >= 1` を契約にする。
  - `MAXARG = 16`, `MAXARGLEN = 128` とし、`MAXARG` 個ちょうど + NULL は成功、`MAXARG + 1` 個は失敗とする。
  - `proc::exec` は `push_argv` で新 user stack に NUL 終端文字列と pointer array を配置し、`a0 = argc`, `a1 = argv_va` として新 program に入る。

## D0035: FD 層は global file table と per-process fd table に分ける

- 日付: 2026-05-07
- 状態: 採用
- 背景: `read` / `write` が fd 0/1/2 を直接特別扱いしていたため、console device、将来の RAM FS / inode / pipe を同じ syscall surface に載せる file abstraction が必要になった。xv6 の `ftable` / `proc.ofile` に近い構造を採るか、process 内に file object を直接持つかを整理した。
- 検討した選択肢:
  - (a) `Process` の fd table に `File` を直接持つ。
  - (b) kernel 全体に global `File` table (`NFILE`) を持ち、process ごとの fd table (`NOFILE`) は `*mut File` を指す。
  - (c) fd 0/1/2 の special-case を残し、FS 実装時に後から file layer を入れる。
- 採用: (b)。
- 理由:
  - xv6 と同じく、fd は process-local な整数、`File` は kernel-global な open file description として分離できる。
  - `fork` や将来の `dup` で同じ open file description を共有できる。inode file の offset なども同じ `File` object に持てる。
  - `NOFILE` は 1 process あたりの fd 上限、`NFILE` は system 全体の opened file object 上限として役割が明確。
  - (a) は `fork` / `dup` で file offset や refcount を共有する設計に後で作り直す可能性が高い。
  - (c) は FS を入れる時点で syscall 層を大きくつなぎ直すことになる。
- 影響:
  - `src/file.rs` を追加し、`File { refcnt, readable, writable, kind }` と global file table を持つ。
  - `FileKind` は当面 `None` と `Device { major }` のみ。kind 固有 field は `FileKind` に置く方針とし、将来の inode offset なども `FileKind::Inode` 側に置く。
  - `FileKind::Device` は当面 `major` だけを持ち、`minor` は file layer では扱わない。console は `CONSOLE_MAJOR` の character device として扱う。
  - `Process` に `ofile: [*mut File; NOFILE]` を追加する。NULL pointer が未使用 fd を表す。
  - `userinit()` は fd 0/1/2 に console device file を割り当てる。stdin は readable、stdout/stderr は writable。
  - `fork()` は親の non-NULL fd entry を child にコピーし、`file::dup` で `refcnt` を増やす。
  - `freeproc()` は残っている fd entry を `file::close` し、`refcnt == 0` になった file slot を unused に戻す。
  - `sys_read` / `sys_write` は fd 範囲と fd table entry を検証し、`file::read` / `file::write` に委譲する。user memory の `copyin` / `copyout` は syscall layer に残す。

## D0036: `write` syscall は short write を許容し、全 byte 保証は user library に置く

- 日付: 2026-05-07
- 状態: 採用
- 背景: `sys_write` は従来、user buffer を chunk loop で copyin して console に全 byte 書いてから `len` を返していた。FD/File 層を入れた後は `write` の対象が console 以外にも広がるため、syscall が全 byte 書き切りを保証するべきか、`read` と同じく実際に処理できた byte 数を返すべきかを整理した。
- 検討した選択肢:
  - (a) xv6 寄りに、kernel が `len` まで loop してできるだけ全部書く。
  - (b) Unix/POSIX API 寄りに、1 回の file write 結果を返し、short write を成功として扱う。
- 採用: (b)。
- 理由:
  - POSIX の `write` は `count` 未満の byte 数を返しても成功であり、全部書く必要がある caller は loop する。
  - `read` syscall も要求 byte 数を必ず満たす保証はなく、実際に読めた byte 数を返す。`write` も同じ形にすると syscall contract が対称になる。
  - pipe / inode / device が増えると short write は自然に発生しうるため、早めに user library 側の `write_all` を使う規約に寄せる。
  - kernel 側は user memory から最大 128 byte を copyin し、`file::write` に 1 回渡すだけで済む。
- 影響:
  - `sys_write` は fd / user buffer を検証し、最大 128 byte を kernel buffer に copyin して `file::write` に渡す。
  - `file::write` が返した実書込 byte 数を syscall return value として返す。失敗は `-1`。
  - user 側で全 byte 出力が必要な箇所は `write_all` を使う。`read_line` の出力も `write_all` に変更する。
  - 直接 `write(fd, large_buf)` を呼ぶ user program は short write を扱う必要がある。

## D0037: 最初の FS は read-only RAM inode FS にする

- 日付: 2026-05-08
- 状態: 採用
- 背景: `exec` や `open/read` のために FS が必要になったが、永続化や write support は当面の目標ではない。一方で、将来 block device backed FS に移るときに syscall / file / exec の上位構造を大きく作り直したくない。
- 検討した選択肢:
  - (a) `FileKind::RamFile { data }` のように RAM FS 固有の file kind を直接 file layer に置く。
  - (b) read-only RAM FS だが、`Inode` / `namei` / `readi` を通す。
  - (c) 最初から virtio-blk / buffer cache / xv6 風 disk inode FS に進む。
- 採用: (b)。
- 理由:
  - `exec` と shell 到達には read-only で十分。
  - write 可能 RAM FS は可変長 data 領域、truncate、途中失敗 rollback などが必要になり、今の目的に対して重い。
  - `namei(path)` と `readi(inode, off, dst)` を通す形にすれば、後で `readi` の内部を block device / buffer cache に差し替えやすい。
  - (a) は早いが、RAM FS 固有の `data` 表現が syscall / file / exec に漏れやすい。
  - (c) は本筋だが、FS より先に block device driver と buffer cache の実装量が大きくなる。
- 影響:
  - `src/fs.rs` に static inode tree を持つ。現時点では `/bin/read_line`, `/bin/read_file`, `/README.md` を登録する。
  - path lookup は絶対 path のみ。`/` は root、末尾 slash / 連続 slash / 相対 path は失敗扱い。cwd / `.` / `..` は未対応。
  - `InodeKind` は private な内部表現にし、外部には `InodeType::{File, Dir, Device}` だけを公開する。
  - file content の読み取りは `fs::readi(inode, off, dst)` に寄せる。RAM 上の `&'static [u8]` を外部へ直接返す API は作らない。
  - regular file への write は未対応で `-1`。console など device だけが `file::write` に成功する。

## D0038: exec loader は inode/readi ベースで ELF を読む

- 日付: 2026-05-08
- 状態: 採用
- 背景: RAM FS の file content は連続 slice として存在するため、`fs::data(inode) -> &[u8]` を作って既存 loader に渡すこともできた。しかし disk-backed FS に進むと file 全体を連続 slice として返すことはできない。`exec` は今後も FS 上の program file を読む中心経路になる。
- 検討した選択肢:
  - (a) RAM FS 固有 API として `fs::data(inode)` を作り、`loader::load_elf(&[u8])` に渡す。
  - (b) `file::read_at` を作り、loader は `File` 経由で offset read する。
  - (c) `loader::load_elf_from_inode` を作り、loader が `fs::readi(inode, off, dst)` で ELF を読む。
- 採用: (c)。
- 理由:
  - `exec` は open fd の offset を使う操作ではなく、path が指す inode から ELF を読む操作なので、xv6 と同様に inode を直接読む方が自然。
  - `fs::data` は RAM FS 依存の抜け道で、disk-backed FS に合わない。
  - `file::read_at` も最終的には `FileKind::Inode` から `fs::readi` に委譲するだけで、現段階では追加抽象になる。
  - ELF header / program header / segment を offset 指定で読む形にしておけば、将来 `readi` の backing store を変えても loader の外側は保ちやすい。
- 影響:
  - `loader::load_elf_from_inode(pt, inode)` を追加する。
  - ELF header / program header は小さい stack buffer に `read_exact_inode` で読み、`read_unaligned` で parse する。
  - PT_LOAD segment は page ごとに `kalloc_zeroed` し、file-backed part だけ `fs::readi` で読む。`memsz > filesz` の残りは zero page のままにする。
  - `sys_exec` は `fs::namei(path)` で inode を引き、`proc::exec_from_inode` に渡す。
  - 旧 embedded program table は不要になり、初期 `init` ELF だけが boot 用に残る。

## D0039: `open` / `close` は read-only RAM FS 向けの最小仕様で始める

- 日付: 2026-05-08
- 状態: 採用
- 背景: read-only RAM FS を user program から確認するため、`open` / `close` syscall が必要になった。flags や permission mode、directory read、device inode の扱いをどこまで入れるかを決める必要があった。
- 検討した選択肢:
  - (a) xv6/POSIX 風に `O_RDONLY` / `O_WRONLY` / `O_RDWR` / `O_CREATE` などを最初から扱う。
  - (b) flags は当面無視し、regular file は read-only、directory は失敗、device は device file として開く。
  - (c) `open` は後回しにし、`exec` だけ FS 経由にする。
- 採用: (b)。
- 理由:
  - FS 自体が read-only なので、write flags を真面目に扱う段階ではない。
  - `open/read/close` の fd table と file offset の動作確認には read-only open で十分。
  - directory read や `O_CREATE` は shell / `ls` / writable FS の段階で改めて設計すればよい。
  - user ABI には `open(path, flags)` の形を置いておき、後で flags を解釈できる余地を残す。
- 影響:
  - syscall 番号は xv6 に合わせて `SYS_OPEN = 15`, `SYS_CLOSE = 21` とする。
  - user 側に `open(path, flags)` / `close(fd)` wrapper と `O_RDONLY = 0` を追加する。
  - `sys_open` は `copyinstr` → `fs::namei` → `file::alloc` → `fdalloc` の順で処理する。途中失敗時は `file::close` で rollback する。
  - `sys_close` は process fd table entry を NULL にしてから `file::close` する。close 後の fd に対する read/write は `-1`。
  - 同じ inode を 2 回 open した場合は別々の `File` object が作られ、offset は独立する。`dup` syscall はまだ未実装。

## D0040: 最初の shell は argv なしの path-only exec にする

- 日付: 2026-05-09
- 状態: 採用
- 背景: D0034 で kernel の `exec` ABI は C 風の argv pointer array を扱えるようにしたが、shell 入力から `argv` を組み立てるには固定長の `&[u8]` 配列や NUL 終端済み buffer の管理が必要になる。まだ user heap が無く、user stack も 1 page だけなので、最初の shell で argv parser まで背負うと目的に対して複雑になる。
- 検討した選択肢:
  - (a) 最初から空白分割 parser を書き、固定長 argv 配列を組み立てて `execv` する。
  - (b) shell は path だけを読み、user library の `exec(path)` が内部で `argv = [path, NULL]` を作る。
  - (c) `exec` から argv support を kernel 側も含めて一旦外す。
- 採用: (b)。
- 理由:
  - 「shell から program を起動する」短期目標に集中できる。
  - heap なし / 1 page user stack の制約下で、user library と shell の固定長 buffer を小さく保てる。
  - kernel 側の argv support は `_start(argc, argv)` や将来の shell 拡張に有用なので残す。
  - `exec(path)` wrapper は path を自動 NUL 終端し、同じ buffer を `argv[0]` として渡せるため、kernel 側の `copy_argv` が要求する `argc >= 1` と整合する。
- 影響:
  - user library の公開 wrapper は当面 `exec(path: &[u8])` を主経路にする。
  - `exec` / `open` の呼び出し側は NUL 終端済み byte string を渡さなくてよい。wrapper が固定長 stack buffer にコピーして NUL 終端する。
  - 最初の `/bin/sh` は入力行を trim したものをそのまま path として扱う。空白分割、quote、escape、cwd、PATH 探索、builtin は未対応。
  - RAM FS の `/bin` は当面 `/bin/sh` と検証用 `/bin/read_file` を持つ。`/bin/read_line` は shell 導入に伴い外す。

## D0041: user stack は `TRAPFRAME` の下の高位固定 VA に配置する

- 日付: 2026-05-11
- 状態: 採用
- 背景: userland allocator / `sbrk` に進むにあたり、従来の「ELF image 直後に user stack を置く」レイアウトでは、ELF 末尾から上に伸びる heap と下向きに伸びる stack の関係が扱いにくい。`sz` の意味も「image + stack を含む address space size」となっており、heap end として使いづらかった。
- 検討した選択肢:
  - (a) 従来通り ELF image 直後に 1 page stack を置く。
  - (b) ELF image 直後を heap start / heap end とし、user stack は `TRAPFRAME` の下に固定配置する。
  - (c) `mmap` 領域なども見越したより本格的な user address space layout を先に設計する。
- 採用: (b)。`USER_STACK = MAXVA - 3 * PGSIZE` とし、1 page stack を `[USER_STACK, USER_STACK + PGSIZE)` に map する。
- 理由:
  - `TRAMPOLINE = MAXVA - PGSIZE`、`TRAPFRAME = MAXVA - 2 * PGSIZE` の直下に user stack を置くと、xv6 風に高位固定 stack と低位 heap を分離できる。
  - ELF image の page-aligned end をそのまま heap start / current break として扱える。
  - heap は低位から上向き、stack は高位から下向きに伸びるため、両者の間に大きな unmapped gap を残せる。
  - `mmap` や grow-on-fault stack はまだ不要で、まずは 1 page fixed stack で十分。
- 影響:
  - `LoadedImage::sz` は stack を含まない。低位 user image の page-aligned end、つまり当面の heap start として扱う。
  - loader は ELF segment を `[0, sz)` に map した後、user stack を `USER_STACK` に別途 map し、`sp = USER_STACK + PGSIZE` を返す。
  - `proc_freepagetable` は `[0, sz)` と固定 mapping (`TRAMPOLINE` / `TRAPFRAME` / 存在する場合の `USER_STACK`) を別々に teardown する。
  - `fork` は `[0, sz)` の copy に加えて、`USER_STACK` page を同じ VA にコピーする必要がある。親子で VA layout は同じなので trapframe の user `sp` は書き換えない。
  - 将来 user stack を複数 page 化、guard page 追加、grow-on-fault 化する場合は、この固定 1 page 方針を再考する。
