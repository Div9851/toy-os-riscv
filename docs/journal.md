# Journal

日付見出しの下に「やったこと / 詰まったこと・わかったこと / 次にやること / 参照」を置く。
設計判断は `docs/decisions.md` に記録し、ここからは `D0001` のように番号で参照する。

---

## 2026-04-29

### やったこと

- README.md / CLAUDE.md を整備し、プロジェクトの方針 (学習目的・ユーザが実装・エージェントは相談役) を明文化。
- ターゲット (`riscv64gc-unknown-none-elf`、QEMU `virt`、OpenSBI、シングルコア) と短期ゴール (シェル起動まで) を確認。
- ブート方式・出力経路・ツールチェイン・ビルドドライバを決定 (D0001〜D0006)。
- 作業ログの運用を決め、本ファイルと `docs/decisions.md` を整備。
- nightly toolchain (`rust-src` / `rustfmt` / `clippy`) と `riscv64gc-unknown-none-elf` ターゲットを導入。
- 環境セットアップの方針を確定: `cargo init --name kernel --bin` で雛形を作り、`Cargo.toml` 編集 + `rust-toolchain.toml` / `.cargo/config.toml` / `Makefile` の 4 ファイルを整備する流れ。
- task runner を cargo runner から Makefile に変更 (D0007 で D0006 を Superseded)。
- 環境整備の残り 3 ファイル (`rust-toolchain.toml` / `.cargo/config.toml` / `Makefile`) を作成。
- `linker.ld` (`ENTRY(_start)`、`. = 0x80200000`、`.text.entry` を先頭、`__bss_start` / `__bss_end`、64 KiB スタック + `__stack_top`) と `src/main.rs` (`#![no_std]` / `#![no_main]`、`global_asm!` による `_start`、`kmain`、`panic_handler`) を作成。
- `make build` → `make run` で `pc = 0x80200038` の spin loop に到達することを確認 (Step 5 完了)。
- SBI Legacy Console Putchar (`a7 = 1`) で `H` を出力 → `"Hello, world!"` を出力 (Step 6/7 完了)。
- `SbiConsole` に `core::fmt::Write` を実装、`print!` / `println!` マクロを定義し、`println!("hartid = {}, dtb = {:#x}", ...)` で `hartid = 0, dtb = 0x87e00000` を確認 (Step 8 完了)。
- `panic_handler` を `println!` 経由のメッセージ表示に更新。
- 短期サブゴール「Hello, world を SBI Console に出す」を完走。

### 詰まったこと / わかったこと

- xv6-riscv は OpenSBI に乗っていない。`entry.S` + `start.c` で M-mode 初期化 (PMP・`medeleg`/`mideleg`・タイマトランポリン) をしてから `mret` で S-mode に降りている。
- OpenSBI に乗ると Console 以外にも以下を抽象化してくれる:
  - タイマ (`sbi_set_timer`) — `mtimecmp` は本来 M-mode 専用なので、自力でやると M-mode トランポリンが必要。
  - HSM (`sbi_hart_start/stop/suspend`) — SMP の bring-up を任せられる。
  - IPI (`sbi_send_ipi`)、System Reset (`sbi_system_reset`)。
- Sv39: 39 bit VA、canonical 制約で上下 256 GiB ずつの 2 領域 (合計 512 GiB)。3 段ページテーブル、ページサイズは 4 KiB / 2 MiB / 1 GiB。物理は理論上 56 bit (PPN 44 bit + offset 12 bit)。
- OpenSBI から飛び込んできた直後の状態: S-mode、`satp = 0`、`a0 = hartid`、`a1 = DTB` 物理アドレス、エントリ `0x8020_0000`。
- `panic` 設定はプロファイル単位。`[profile.dev]` と `[profile.release]` の両方に `panic = "abort"` を書く必要がある。`test` / `bench` プロファイルは書いても常に `unwind` (テストハーネスが unwind を要求するため)。
- `cargo init` は `Cargo.toml` / `src/main.rs` / `.gitignore` を作るが、`rust-toolchain.toml` と `.cargo/config.toml` は cargo の管轄外なので手で作る必要がある。
- no_std + Custom Test Framework (`#![feature(custom_test_frameworks)]`) を使えば `cargo test` を QEMU 上で走らせる仕組みは組める (blog_os 流) が、最初は Makefile + script で十分。
- cargo runner は 1 ターゲットにつき 1 つしか書けないので、debug 起動・gdb 接続・objdump など起動の変種が増える kernel 開発では Makefile のほうが素直。
- VS Code の rust-analyzer は古い toolchain を掴んだままになることがある。`rust-toolchain.toml` 追加直後は **"rust-analyzer: Restart Server"** で読み直しが必要。エラー文 (`target may not be installed`) は誤報。
- no_std で rust-analyzer が `--all-targets` 相当を裏で走らせると、`test` クレートを要求して失敗する。`Cargo.toml` の `[[bin]] test = false, bench = false` で抑止できる (本格的な kernel テストは Custom Test Framework か外部 script に進むときに別途設計)。
- edition 2024 では `#[no_mangle]` が `#[unsafe(no_mangle)]` に。`extern "C"` 関数を asm から呼ぶ kernel コードでは必須。
- SBI ABI: `a7 = EID, a0..a5 = 引数`、`ecall` で M-mode (OpenSBI) にトラップ。Legacy Console Putchar は `EID = 1`、戻り値は `a0` に来るので `lateout("a0") _` で捨てる。
- `core::fmt::Write` は `write_str(&str)` 1 つだけを実装すれば、`write!` / `writeln!` のフル機能が手に入る。中で組み立てられる `core::fmt::Arguments<'_>` はヒープレスなフォーマット表現で、`no_std` 環境にちょうどよい。
- `print!` / `println!` の自前マクロは `$crate::SbiConsole` 参照 + `#[macro_export]` で書く。`$crate` のおかげでクレート分割しても破綻しない。
- QEMU virt + OpenSBI 1.5 環境での DTB 物理アドレスは `0x87e00000` 付近 (128 MiB RAM の上端近く)。
- `_start` (`la sp` + `la t0/t1` + BSS clear loop + `tail kmain`) のサイズは 0x38 程度。なので kmain の入口は `0x80200038` 付近に来る。

### 次にやること

短期サブゴール「Hello, world」は完了。次に何を進めるかは相談ベース。候補:

- (a) **コンソール周りをモジュール化** — `src/console.rs` に `SbiConsole` / `print!` / `println!` を切り出し、`main.rs` を綺麗に保つ。
- (b) **直 16550 UART に書き直す** — D0003 の「後で書き直す予定」を消化。MMIO の最初の一歩。
- (c) **トラップ・割り込みハンドラの最低限** — `stvec` を設定し、例外発生時に `scause` / `sepc` / `stval` をダンプ。以降の開発で kernel が落ちたとき何が起きたか分かるようになる。
- (d) **DTB をパース** — memory ノードを読んで RAM 範囲を把握。物理ページアロケータの下準備。
- (e) **物理ページアロケータ** — bump / freelist。続いて Sv39 ページング。

学習効果と次へのつながりで言うと **(c) → (d) → (e)** の順が王道。(a) はいつ挟んでも良い軽作業。(b) は学習目的としては寄り道だが、「OS が直接 UART を喋る」感は得られる。

### 参照

- RISC-V Privileged Spec — Supervisor-Level ISA (Sv39 / `satp`)。
- RISC-V SBI Spec — Legacy Extension の Console Putchar (EID = `0x01`)。
- xv6-riscv の `kernel/entry.S`、`kernel/start.c` (今回は採用しないが参考)。
