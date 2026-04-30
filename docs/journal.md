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

順番は **(a) → (b) → (c) → (e)** で進める ((d) は D0008 でハードコード採用によりスキップ)。

1. **(a) コンソール周りをモジュール化** — `src/console.rs` に `SbiConsole` / `print!` / `println!` を切り出し、`src/memlayout.rs` に `KERNBASE` / `PHYSTOP` / `UART0` / `PLIC` / `CLINT` / `VIRTIO0` 等を定数で配置 (D0008)。コンソールはトレイトで抽象化しておくと (b) で差し替えやすい。
2. **(b) 直 16550 UART に書き直す** — `0x10000000` の MMIO 直書きで実装。SBI Console から自前ドライバへ。MMIO デバイスの最初のひな型。
3. **(c) トラップ・割り込みハンドラの最低限** — `stvec` を設定し、トラップフレームを保存。例外発生時に `scause` / `sepc` / `stval` をダンプ。落ちたとき何が起きたかが見えるようになる。
4. **(e) 物理ページアロケータ** — `[KERNBASE, PHYSTOP)` を 4 KiB ページ単位で freelist 管理。

ここから先は D0009 (init 1 個先行) / D0010 (identity map) / D0011 (init 埋め込み) に従う:

5. **(f) Sv39 ページングを identity map で有効化** — カーネル RAM + MMIO を identity マップしたカーネルページテーブルを組み、`satp.MODE = 8` で切り替え。`sfence.vma` で TLB をフラッシュ。
6. **(g) ユーザページテーブル + 最初の U-mode 遷移** — init 用のページテーブルを組み、`sstatus.SPP = 0` / `sepc = entry` を設定して `sret` で U-mode へ。
7. **(h) syscall ABI の雛形** — `ecall` を `scause = 8` (Environment call from U-mode) としてハンドル。最低限 `write` / `exit` のみ。引数は `a0..a5`、syscall 番号は `a7`、戻り値は `a0` (Linux/xv6 と同じ慣例で良いか別途検討)。
8. **(i) init を埋め込み ELF としてロード** — `include_bytes!` で取り込んだ ELF をパースし、ユーザページテーブルにマップ。エントリポイントへ `sret`。

UART RX 割り込みによるキーボード入力 (= shell の getchar の下準備) は (c) の後に別途。スケジューラ・`fork` は (i) が動いてから着手する (D0009)。

### 参照

- RISC-V Privileged Spec — Supervisor-Level ISA (Sv39 / `satp`)。
- RISC-V SBI Spec — Legacy Extension の Console Putchar (EID = `0x01`)。
- xv6-riscv の `kernel/entry.S`、`kernel/start.c` (今回は採用しないが参考)。

---

## 2026-04-30

### やったこと

- D0009〜D0011 をコミットに反映 (前日決定分)。
- (a) コンソールのモジュール化方針を再考し、SBI Console を撤去して 16550 UART 直叩きに統一する方向に変更 (D0012)。旧 (a) と (b) は 1 ステップに統合。
- Console 出力は最初から `spin::Mutex` で保護する方針を決定 (D0013)。
- UART 初期化は xv6 風の最小手順を踏む (LCR で DLAB → DLL/DLM → LCR で 8N1 → FCR で FIFO enable + clear)。
- (a) を完了: `src/memlayout.rs` (`KERNBASE` / `PHYSTOP` / `UART0` / `CLINT` / `PLIC` / `VIRTIO0` / `__kernel_end`) / `src/uart.rs` (Uart16550 + xv6 風 init + busy-wait `putc`) / `src/console.rs` (`static CONSOLE: spin::Mutex<Uart16550>` + `print!`/`println!` + panic 用ロックレス経路) を作成。`linker.ld` に `__kernel_end = .;` を追加。SBI Console を撤去し、UART 直叩きで `Hello from UART!` 出力を確認。
- (c) トラップハンドラのミニマル版を実装。`src/trap.rs` に `init()` (stvec 設定)、`#[unsafe(naked)] extern "C" fn trap_entry()` (31 本退避 + `call kerneltrap` + 復帰 + `sret`)、`extern "C" fn kerneltrap()` (scause/sepc/stval/sstatus を読んで `panic!`) を配置。`kmain` から `trap::init()` 後に `unimp` を発火させ、`scause = 0x2` (Illegal instruction)、`sstatus.SPP = 1` を観測。
- xv6 の kernelvec 流に倣い、`struct trapframe` 型は定義せず、asm の 256 バイトスタック領域 + C ローカルの sepc/sstatus 退避という分担をそのまま採用 (D0014)。

### 詰まったこと / わかったこと

- `&dyn Trait` は no_std でも普通に使える。実体は fat pointer (data ptr + vtable ptr の 2 ワード)。アロケーション不要、コストはほぼ間接呼び出し分のみ。
- xv6-riscv の `printf` は専用 spinlock を持ち、`acquire`/`release` 中で `push_off`/`pop_off` により割り込みを禁止する。`panic` 時は `pr.locking = 0` でロックを取らない出力経路に切り替える。
- QEMU `virt` + `-m 128M` は「カーネルとは別に 128 MiB」ではなく「ゲスト RAM 全体が 128 MiB (`0x8000_0000`〜`0x8800_0000`)」。OpenSBI もカーネルもこの中に置かれる。アロケータが自由に使えるのは `[__kernel_end, PHYSTOP)` の差し引き分 (約 125 MiB)。
- QEMU virt の 16550 UART: ベース `0x1000_0000`、PC16550 互換。送信は `LSR` (offset 5) の bit 5 (`THRE`) を待って `THR` (offset 0) に書く。QEMU はエミュレーションが緩く init 無しでも動くが、実機相当の最小 init を踏む。
- OpenSBI のデフォルト PMP 設定では S-mode から MMIO (UART / CLINT / PLIC) に直接アクセスできる。Console を 16550 直叩きにしても権限上の問題はない。
- 16550 のレジスタは offset 0/1 の意味が DLAB ビット (LCR bit 7) で切り替わる。DLAB=0: offset 0 = THR/RBR、offset 1 = IER。DLAB=1: offset 0 = DLL、offset 1 = DLM (baud divisor latch)。baud を書きたいときだけ DLAB を ON にする。
- 16550 UART の参考資料の優先度: (a) xv6-riscv `kernel/uart.c` (実装の最良の参考、100 行未満)、(b) OSDev wiki "Serial Ports"、(c) PC16550D データシート §6/§7 (権威ある定義)、(d) QEMU `hw/char/serial.c` (エミュレーションの実態確認)。
- UART IER で TX/RX 割り込みを ON にしても、CPU まで届くには **PLIC enable / `sstatus.SIE` / `sie.SEIE`** の 3 段の関門を越える必要がある。stvec / PLIC 未設定の段階で IER を立てても、PLIC が UART IRQ を CPU に転送せず SIE も off のため割り込みは起きない。よって xv6 と同じく `uartinit` 末尾で `IER_TX_ENABLE | IER_RX_ENABLE` を立ててしまって問題ない。
- xv6 の `uart.c` にある `tx_lock` は **TX リングバッファ** (`uart_tx_buf`) を守るためのロック。printf の直列化は別途 `pr.lock` (printf.c) が担当している。我々は busy-wait の sync TX のみでリングバッファを持たないため `tx_lock` 相当は不要。装置の直列化は console 側の `Mutex<Uart16550>` で足りる。非同期 TX (`write` syscall がリングに enqueue → TX 完了割り込みで drain) を実装する段階で初めて 3 点セット (`tx_lock` / `uartputc` / `uartputc_sync`) が必要になる。
- MMIO アクセスは必ず `core::ptr::read_volatile` / `write_volatile` を使う。普通の `*ptr = ...` は LLVM が並べ替え/削除する可能性があり、副作用を持つレジスタアクセスでは正しく動かない。16550 のレジスタは 1 バイト単位で叩く (`u32` で叩くと QEMU では動いても実機では未定義動作)。
- panic 経路で「ロックを取らない出力」を実現するには、`Mutex::force_unlock` (他者が握ったロックを横取り、UB の温床) ではなく **MMIO アドレスは固定なので別インスタンスを作って叩く** 方式を採る。物理的に同じデバイスに書けるので機能的には等価で、Mutex の状態に触れないため安全。
- rust-analyzer が「This file is not included anywhere in the module tree」を出すのは、その `*.rs` ファイルが親モジュール (= `main.rs`) で `mod` 宣言されていないため。`src/foo.rs` を作ったら `main.rs` に `mod foo;` を 1 行加えれば認識される。新規ファイルは中身を書く前に先に `mod` 宣言しておくと補完が効く状態で書ける。
- 2024 edition では `extern "C" { ... }` ブロックも `unsafe extern "C" { ... }` が必須 (RFC 3484)。`#[no_mangle]` → `#[unsafe(no_mangle)]` と同じ流れ。`unsafe extern` ブロック内の `static`/`fn` 宣言はデフォルトで safe (利用側で `unsafe` 不要)。利用側にも `unsafe` を要求したい場合は宣言にも `unsafe static` / `unsafe fn` を付ける。リンカシンボルのアドレスを取りたいだけなら前者 (デフォルト safe) で十分。
- `naked_functions` は Rust 1.88 で stabilize 済み。feature gate は不要、`#[unsafe(naked)]` 属性 + `core::arch::naked_asm!` マクロで書ける。naked function の本体は `naked_asm!` を 1 個呼ぶだけというルールで、普通の `asm!` を書くとコンパイルエラー。prologue/epilogue を一切付けない契約なので、スタック調整・呼び出し・復帰・`sret` まで全部 asm 側の責任。
- 関数アイテム型を直接 `as usize` する書き方は警告 (`direct cast of function item into an integer`) が出る。関数アイテムはゼロサイズ型で、暗黙に関数ポインタへ coerce してから整数化する 2 段の変換が混ざるため。`let f: extern "C" fn() -> ! = trap_entry; f as usize` のように関数ポインタを 1 段挟むのが推奨。
- `extern "C" fn` は呼び出し規約 (psABI) を C ABI に固定する宣言。引数 `a0..a7` / 戻り値 `a0,a1` / callee-saved `s0..s11,sp,gp,tp` が保証される。asm から呼ぶ関数 (`kerneltrap`) と CPU から呼ばれる関数 (`trap_entry`) の両方で必要。
- `stvec` は下位 2 bit が MODE (0 = Direct, 1 = Vectored)、上位が BASE (4-byte aligned)。Direct + 4-byte aligned なアドレスを書けば MODE bits が自然に 0 になるので、アドレスをそのまま `csrw` するだけで済む。`naked_asm!` 冒頭の `.align 2` (= 4-byte 境界) で trap_entry 側の alignment を担保。
- `unimp` は rv64gc では圧縮形 `c.unimp` (= `0x0000`、2 バイト) としてアセンブルされる。観測した `sepc = 0x802012a6` の末尾が 2-byte 境界であることが裏取り。
- `stval` は illegal instruction では implementation-defined (命令ビット列を入れる or 0)。QEMU + `c.unimp` の組み合わせでは 0 が観測された。
- `sstatus` の SD bit (bit 63) と FS = Dirty (bits 13-14) が立っているのが目につくが、現状 FP は使っていないので実害なし。注目すべきは SPP (bit 8) = 1 で、これが「S→S トラップ」が成立した直接の証拠。

### 次にやること (4/29 の節を更新)

進捗 (2026-04-30 夜): **(a) と (c) のミニマル版まで完了**。`push_off`/`pop_off` と Mutex の割り込み禁止連携は (c') または別ステップに後送り。

旧 (a) と (b) を統合し、以下の順で進める:

1. ~~**(a) Console を 16550 UART 直叩きで実装、モジュール化** (D0012, D0013)~~ — **完了**。
2. ~~**(c) トラップハンドラの最低限**~~ — **ミニマル版完了**。`stvec` 設定 / 31 本退避 / `scause`/`sepc`/`stval`/`sstatus` ダンプ → panic、まで通った。割り込み (タイマ・PLIC) と `push_off`/`pop_off` は (c') に分離。
3. **(c') 割り込みの導入** — タイマ割り込み (`sbi_set_timer` + `sie.STIE` + `sstatus.SIE`) と / または PLIC + UART RX を有効化し、`kerneltrap` で interrupt vs exception を振り分け。Console Mutex に `push_off`/`pop_off` (ロック区間中の割り込み禁止) を導入し、再入 deadlock を防ぐ。panic 経路はすでに lockless にしてあるのでここでは触らない。
4. **(e) 物理ページアロケータ** — `[__kernel_end, PHYSTOP)` を 4 KiB ページ単位で freelist 管理。
5. **(f) Sv39 ページングを identity map で有効化** — カーネル RAM + MMIO を identity マップしたカーネルページテーブルを組み、`satp.MODE = 8` で切り替え。`sfence.vma` で TLB をフラッシュ。
6. **(g) ユーザページテーブル + 最初の U-mode 遷移** — init 用のページテーブルを組み、`sstatus.SPP = 0` / `sepc = entry` を設定して `sret` で U-mode へ。
7. **(h) syscall ABI の雛形** — `ecall` を `scause = 8` としてハンドル。最低限 `write` / `exit` のみ。
8. **(i) init を埋め込み ELF としてロード** — `include_bytes!` で取り込んだ ELF をパースしてユーザページテーブルにマップ、`sret` でエントリへ。

(c') と (e) はどちらを先にしても (f) には到達できる。割り込みの全体像 (PLIC + sip/sie + UART IRQ + push_off) を先に通すか、メモリ管理を一直線に進めるかは別途判断。スケジューラ・`fork` は (i) が動いてから (D0009)。

### 参照

- xv6-riscv `kernel/printf.c` (専用 spinlock + panic 時の lockless 経路)、`kernel/uart.c` (`uartinit`、LCR / FCR の使い方)。
- xv6-riscv `kernel/kernelvec.S` (S→S トラップ入口の参考実装)、`kernel/trap.c::kerneltrap()` (CSR の C ローカル退避と devintr 振り分け)。
- PC16550 datasheet (8250/16450/16550 系の標準レジスタ配置)。QEMU 実装は `hw/char/serial.c`。
- spin crate (`spin::Mutex`、`Once`)。
- RISC-V Privileged Spec — `stvec` (§4.1.2)、`scause` / `sepc` / `stval` / `sstatus` (§4.1.6 〜 §4.1.8)、Trap Cause encoding (Table 4.2)。
- Rust Reference — Inline Assembly、Naked Functions (Rust 1.88 で stabilize)。

---

## 2026-05-01

### やったこと

- (c'-1) タイマ割り込みのミニマル版を実装。`src/timer.rs` に `TICK: AtomicU64`、`pub fn init()` (sbi_set_timer + sie.STIE + sstatus.SIE)、`pub fn handle()` (TICK インクリメント + 次 deadline 設定)、private な `rdtime` / `sbi_set_timer` / `schedule_next` を配置。
- `kerneltrap` を dispatch 化。`scause` の MSB で interrupt / exception を分け、interrupt 側で code = 5 (Supervisor timer) を `timer::handle` に振る。それ以外の interrupt と全ての exception は引き続き panic。
- `kmain` で `TICK` を polling し、1 秒ごとに `tick N` が出力されることを観測 ((c'-1) のゴール達成)。
- 動作確認の積み方: dispatch 化 → タイマ有効化のみで「code = 5 で panic」→ handle 繋ぎ込みで「panic 消える」→ TICK polling で「tick が見える」、と 4 段で切り分けた。
- (c'-2) `push_off` / `pop_off` + 自前 Spinlock を実装。`src/cpu.rs` に `Cpu { noff, intena }` を static で 1 個持ち、`push_off` / `pop_off` / `mycpu` / `intr_get` / `intr_off` / `intr_on` を実装。`src/spinlock.rs` に `Spinlock<T>` + `SpinlockGuard<T>` を実装し、`Drop` で release → `pop_off` の RAII にした。
- `console.rs` を `spin::Mutex<Uart16550>` から自前の `Spinlock<Uart16550>` に置き換え。`spin` crate を `Cargo.toml` から削除。
- `timer::handle()` から `println!("tick N")` を直接呼ぶ形に変更し、再入 deadlock せずに 1 秒ごとに出力されることを確認 ((c'-2) のゴール達成、D0015)。
- xv6 の `holding(lk)` 相当の self-deadlock check は今回省略 (再帰取得時は無限 spin する)。必要になったら後付けで入れる。

### 詰まったこと / わかったこと

- QEMU virt の `mtime` は 10 MHz で増加 (CLINT の timebase)。1 秒 = `10_000_000` tick。INTERVAL は 1 秒で見やすい。
- `csrs` / `csrc` は CSR の指定ビットだけを atomic に立てる/落とす擬似命令 (実体は `csrrs x0, csr, rs` / `csrrc x0, csr, rs`)。`csrw` は全体置き換えなので、他のビットを残したい場面では `csrs` / `csrc` を使う。即値版 `csrsi` / `csrci` は 5-bit (0..31) のみ。bit 5 (= 32) 以上を立てたいときは register 経由が必要。
- SBI ABI 呼び出しでは特定レジスタ名指しが必須: `in("a7") EID`, `in("a6") FID`, `in("a0") arg`。`lateout("a0") _` は「`a0` への in と out を同時に書く」ための指定で、`in("a0") x` と並べて使える (`a0` は input としても output としても使われる)。
- `rdtime` は擬似命令、実体は `csrr rd, time`。rv64 では 1 命令で 64 bit 読める。OpenSBI のデフォルトでは S-mode から `time` CSR が読める。
- 割り込みを有効化する順序は「配線してから電源を入れる」: (1) `sbi_set_timer` で deadline → (2) `sie.STIE = 1` → (3) `sstatus.SIE = 1`。逆順だと `sstatus.SIE = 1` の瞬間に他の sie ビットで配線済みの割り込みが暴発しうる。
- `scause` の **例外コードと割り込みコードは別の番号空間**。例えば code = 5 は exception だと Load access fault、interrupt だと Supervisor timer。`is_interrupt` で分けてから code を見るのが必須。
- 割り込みハンドラから戻る `sret` 経路は (c) で検証済みの 31 本退避と同じ。タイマでも S→S は同じ asm 入口を共有する。
- push_off / pop_off は今ステップでは入れず、ハンドラから `println!` を呼ばないことで再入 deadlock を回避。代わりに `AtomicU64` カウンタ + メイン側の polling で観測した。
- `Spinlock` を自前で書くには 4 点の道具が要る: (1) `UnsafeCell<T>` で `&self` から内部 `&mut T` を取り出す経路、(2) `AtomicBool` でロックフラグ、(3) `unsafe impl<T: Send> Sync` で複数 hart 間共有を許可、(4) `SpinlockGuard` の `Drop` で release。
- `Ordering::Acquire` / `Release` は critical section の中身が外に並べ替えられないようコンパイラ・CPU を縛る。`Relaxed` だと最適化で漏れ出てロックの意味が消える。Mutex 系は **必ず Acquire/Release ペア** で書く。
- `push_off` は `swap` の前、`pop_off` は `store(false)` の後に呼ぶ。順序が逆だと「ロック保有 ↔ 割り込み許可」の谷間が開いてハンドラから再取得 → deadlock の窓ができる。
- `Deref` / `DerefMut` を `SpinlockGuard` に実装すると、`*g` で中身に届く + `g.method()` が中身のメソッドに透過する (auto-deref)。明示的な `get()` を呼ばなくて済むので利用側がきれい。
- `Drop` + `Deref` で **RAII パターン**: Guard を作る = 取得、スコープ終了 = 解放。明示的な unlock 不要、解放忘れがコンパイラレベルで起きない。C++ の `std::lock_guard` を型システムで強制する形。
- `unsafe impl<T: Send> Sync for Spinlock<T>`: `Send` は所有権移動可、`Sync` は共有参照アクセス可。Mutex は中身を 1 thread ずつにロック越しに渡す仕組みなので、`T: Send` で十分 `Sync` を名乗れる (標準ライブラリの `std::sync::Mutex` も同じ宣言)。
- `intr_get` / `intr_off` / `intr_on` は `sstatus.SIE` (bit 1) を `csrr` / `csrc` / `csrs` で読み書きするだけ。bit 5 の `sie.STIE` (= タイマ個別有効化) と混同しないこと。
- `static mut CPU`: Rust 2024 edition では `static mut` への参照取得に `unsafe` が必須。`addr_of_mut!` 経由で `&mut *` する作法。シングルコア前提なので排他は割り込み禁止で確保。SMP 化時は hartid 配列化が必要。
- `push_off` 内の「`intr_get` → `intr_off`」の間に割り込みが入る微小窓があるが、ハンドラが push_off / pop_off を balanced に呼ぶ限り `noff` / `intena` は復元され、整合する (xv6 と同じ作法)。

### 次にやること

- ~~**(c'-2) `push_off` / `pop_off` + 自前 Spinlock の導入**~~ — **完了** (D0015)。
- **(c'-3) PLIC + UART RX**。priority/threshold/enable/claim/complete のサイクルを通し、UART の IER で受信割り込みを有効化、キー入力を `kerneltrap` 経由で受け取る。次に取り組むタスク。

### 参照

- RISC-V SBI Specification — Timer Extension (EID = `0x5449_4D45` "TIME", FID = 0)。Legacy Timer Extension (EID = `0x00`) も等価機能。
- RISC-V Privileged Spec — `sie` / `sip` / Supervisor Timer Interrupt の番号付け (Table 4.2: interrupt code 5)。
- xv6-riscv `kernel/trap.c::devintr()` (interrupt 種別の振り分け)、`kernel/start.c::timerinit()` (M-mode で mtimecmp を直接設定する版、参考)。
- xv6-riscv `kernel/spinlock.c` (`acquire` / `release` / `push_off` / `pop_off`)、`kernel/proc.h` (`struct cpu`)。
- Rust std — `std::sync::Mutex` の `Sync` 境界 (`unsafe impl<T: Send> Sync`)、`MutexGuard` の Deref/DerefMut/Drop パターン。
- Rust Reference — `UnsafeCell`、`addr_of_mut!`、`static mut` の 2024 edition での扱い。
