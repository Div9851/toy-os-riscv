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
