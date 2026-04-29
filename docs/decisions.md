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
