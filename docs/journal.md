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
- (c'-3) PLIC + UART RX の経路を実装。`src/plic.rs` を新規作成、`init()` で UART_IRQ (= 10) を priority 1 / threshold 0 / enable し、`sie.SEIE` も立てた。`uart.rs` に `getc()` を追加、`init()` の末尾で `IER` の RX enable ビット (bit 0 = ERBFI) を立てた。
- `kerneltrap` の dispatch に code = 9 (Supervisor external) を追加し、`plic::handle_external` で claim → IRQ 振り分け → complete のサイクルを実装。UART_IRQ なら `getc()` で受信バイトを読み、`rx: 0xNN 'x'` で表示。
- キー入力で `rx: ...` が `tick N` と並走して表示されることを確認 ((c'-3) のゴール達成)。
- 同 hart 再帰ロック取得を避けるため、`uart_rx()` 内で `CONSOLE.lock()` をブロックで囲んで `getc()` の結果だけ取り出し、guard を drop してから `println!` を呼ぶ書き方を採用。RAII の典型イディオム。
- (e) 物理ページアロケータを実装 (D0016)。`src/kalloc.rs` に xv6 風 freelist (`Run` / `KMem` / `Spinlock<KMem>`) を配置。`init` / `freerange` / `kfree` / `kalloc` を実装。`kfree` は (1) ページ境界・範囲 assert、(2) `0x05` で junk fill、(3) freelist push の 3 段。
- アドレス型 `PhysAddr` / `VirtAddr` を newtype で導入 (D0017)。`src/memlayout.rs` に `PGSIZE` / `PGSHIFT` と一緒に配置。`PhysAddr` のメソッドは `Copy` 型の慣習に従い `self` レシーバ。MMIO 系の定数 (`KERNBASE` / `UART0` 等) は `usize` のまま据え置き。
- `linker.ld` で `__kernel_end` を 4 KiB align、`kalloc::init` 側はコード側 round_up を持たず assert で確認する形に統一。依存先を双方向にコメントで明示。
- グローバル割り込み有効化を `kmain` に集約 (D0018)。`timer::init` から `sstatus.SIE = 1` を削除、`cpu::intr_on` を `pub` に、`kmain` 末尾で `intr_on()`。
- 動作確認: `kalloc()` を空になるまでループして `page count = 32234` を観測 (= `(PHYSTOP - __kernel_end) / 4 KiB`、`__kernel_end ≈ 0x80216000`)。LIFO 確認として `kfree(p1)` 直後の `kalloc()` で `p1` が返ることも確認。
- (f) Sv39 ページングを identity map で有効化。`src/vm.rs` を新規作成。
  - `Pte` (newtype, bit 10 から 44 bit PPN)、`PageTable` (`#[repr(C, align(4096))]` + コンパイル時 size assert)、`walk` (3 段降下、不在中間 PT は kalloc + ゼロクリア + `Pte::new_table` で生やす、megapage 防御として中間 PTE が leaf なら `None`)、`mappages` (page-aligned 引数を assert、`while va < last` 形、double-map で `Err`)。
  - `kvmmake` でカーネル PT を構築。`kvmmap` (size 指定) と `kvmmap_range` (区間指定) の 2 ヘルパで MMIO 群と linker 区間を分けて呼び分ける形。
  - **W^X 分離** を採用 (D0019)。`linker.ld` に `__etext` / `__erodata` を 4 KiB 境界で追加し、`[KERNBASE, __etext)` を RX、`[__etext, __erodata)` を R、`[__erodata, PHYSTOP)` を RW で識別マップ。MMIO (UART / CLINT / PLIC) は RW のみ (X 不要)。
  - PTE の **A / D bit を `Pte::new_leaf` で強制 OR** (D0020)。退避を実装しない学習段階では恒久的に立てておくのが Svade / Svadu いずれの実装でも安全 (= A/D 起因の page fault が原理的に出ない)。
  - `cpu.rs` に `r_satp` / `w_satp` / `sfence_vma` を追加 (CSR ラッパは cpu に集約 D0021)。`vm::kvminithart` で `sfence.vma` → `csrw satp (MODE=8)` → `sfence.vma` の 3 命令で切り替え。
  - 動作確認: 切り替え後の `paging on` 出力 + `tick N` 継続 + UART RX 継続で、データ・命令フェッチ・割り込み経路すべてが新しい satp 越しに正しく走ることを確認。
  - 副産物としてカーネルレイアウト実測値が見える: `.text` 20 KiB (5 ページ)、`.rodata` 12 KiB (3 ページ)、残り `[__erodata, PHYSTOP) ≈ 125 MiB` が RW (data + bss + stack + free pages)。

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
- PLIC は (priority [IRQ 軸], threshold [context 軸], enable [context × IRQ ビットマップ], claim/complete [context 軸]) の 4 種類で構成。アドレス計算が二軸なので一度整理が要るが、シングルコア固定なら `Senable = PLIC + 0x2080` / `Sthreshold = PLIC + 0x20_1000` / `Sclaim = PLIC + 0x20_1004` の 3 定数で済む。
- "context" は (hart, 特権モード) のペア。hart 0 では M-mode = ctx 0、S-mode = ctx 1。Enable / Threshold / Claim はコンテキスト単位なので、S-mode と M-mode で別領域を持つ。
- claim と complete は **同じ MMIO アドレス** で、read = claim (返り値: IRQ 番号、内部 pending クリア)、write = complete (引数: IRQ 番号、次回発火を許可)。順序は claim → デバイス処理 → complete を守る。
- claim が 0 = 「保留中なし」(spurious)。`Option<u32>` で受けて `None` なら早期 return が xv6 流。
- UART は IER の bit 0 (ERBFI) で RX 割り込みを許可する。`LCR.DLAB = 0` のときに書く必要があるので、LCR 最終設定 (8N1, DLAB=0) の後に立てる。
- `sie.SEIE` は bit 9 (`sip.SEIP` と同じ位置)。`csrs sie, 1<<9` で他の sie ビット (= STIE) を残しつつ立てる。
- 自前 Spinlock に self-deadlock check が無いので、同 hart 再帰取得すると無限 spin する。`uart_rx()` で `CONSOLE.lock().getc()` のあと `println!` を呼ぶ書き方は **再帰取得** になるので、ブロックスコープで lock guard を drop させる必要がある。
- UART は RBR を読むと自動で IRQ ライン下げ + RX FIFO から 1 バイト pop する。FIFO に複数バイト溜まっている可能性があるので `while getc().is_some()` で読み切るのが安全。
- xv6 流 freelist は **空きページの先頭 8 バイトに `next` を書く** ことでメタ領域不要にしている。`kfree` 中で `(*pa).next = head; head = pa` するため identity map 必須 (D0010)。
- `*mut T` を含む構造体は `!Send` / `!Sync`。`static Spinlock<KMem>` を成立させるには `unsafe impl Send for KMem {}` が必要 (`Spinlock<T: Send>: Sync` の境界に乗せる)。SMP 化時に妥当性を再評価。
- junk fill のタイミング: `kfree` は (assert → fill → push) の順。逆だと `next` を書いた直後に fill が踏み潰す。fill 自体は lock の外でも安全 (このページはまだ freelist に publish されておらず、Spinlock の Release が pre-store を後続の Acquire 側に見せる)。
- `kalloc` は **lock 取得後に `(*r).next` を読む**。lock 外で読むと前の `kfree` が Release で publish した値を Acquire 越しに読めずデータ競合。
- `Copy` 型のメソッドレシーバは `&self` でなく `self` が Rust 慣習 (`NonZeroUsize` 等の標準ライブラリも同じ)。`PhysAddr` も統一。
- `linker.ld` の `. = ALIGN(4096); __kernel_end = .;` は kalloc::init から見ると暗黙の契約。コード側 round_up を持つか linker.ld 依存 + assert で守るかは設計判断。今回は後者 + 双方向コメントで残す。
- `timer::init` 内の `sstatus.SIE = 1` は関数名から読めない隠れた副作用だった。グローバル enable と個別 enable は責務が違うので場所を分ける (D0018)。
- `kalloc::init` 内では `KMEM.lock()` を 32234 回取るが、`intr_on()` を kmain 末尾に置くため init 中は SIE off。`push_off`/`pop_off` も intena=false を保存→復帰のみで追加コストほぼ無し。
- `page_round_up` は `usize::MAX` 近傍でオーバフローし得るが、`PHYSTOP = 0x8800_0000` 程度しか渡さない用途なので今回は対応せず。
- Sv39 PTE は **PPN を bit 10 から 44 bit 幅で** 並べる (= `(pa >> 12) << 10`)。bit 12 から書くと壊れる。逆向き取り出しは `((pte >> 10) & ((1<<44)-1)) << 12`。マスクを忘れると Reserved bit が将来拡張で立ったとき壊れる。
- 中間 PTE (= 次レベル PT へのポインタ) は **R=W=X=0、V のみ立てる**。leaf PTE との区別はこの 3 bit で付く (`is_leaf = is_valid && (R|W|X != 0)`)。
- 新しく kalloc した中間 PT は **必ずゼロクリア**。kfree の junk fill (`0x05`) のままだと `is_valid()` が真と解釈されて暴走する。`walk` 内 + root を作る呼び側の両方で責任を持つ。
- A / D bit の意味: A = アクセス済み (CPU が読/書/実行で立てる)、D = 書き込み済み (CPU が書きで立てる)。OS は LRU 近似 / writeback / COW のヒントとして使う。
- A / D の更新方式は仕様で 2 通り定義 (Svade = OS 責任で fault、Svadu = HW が atomic に立てる)。QEMU は Svadu、SiFive 系実機は Svade のものが多い。退避を実装しない設計なら **OS 側で常時 1 を書いておけばどちらでも fault しない**。情報量の損失は無い (使う場面が無いので)。
- `mappages` のループ終端を `va < last` 形 (`last = page_round_up(va + size)`) にすると、xv6 の `va <= last` (`last = page_round_down(va + size - 1)`) と等価で、off-by-one の窓が減る。
- identity map のおかげで `csrw satp` 直後の数命令が古い TLB で実行されても問題が起きない (新旧の写像が一致)。higher-half にすると切り替え瞬間に PC を貼り替えるトランポリンが要る (D0010 で identity を選んだ理由の実体験)。
- `sfence.vma` は satp 書き込みの **前と後の両方** で打つ。前: 古い TLB を消す (対称形、scheduler 経由のユーザ PT 切り替えで活きる)、後: 新しい satp の効果を可視化 (これが無いと CPU が古い PT で歩き続ける可能性あり、仕様で必須)。
- `linker.ld` でセクション間を `ALIGN(4096)` で区切ることが W^X の前提。`.text` セクション内で末尾 `ALIGN(4096); __etext = .;` すれば、`.text` 自体の長さが次のページ境界まで伸びる (= rodata が text と同居しない)。境界 1 つあたり最大 4 KiB-1 のパディングが発生するが、合計でも 12 KiB 以下で誤差。
- `satp` の RV64 レイアウト: bit 63-60 = MODE (Sv39 = 8)、bit 59-44 = ASID、bit 43-0 = PPN (44 bit)。PT の物理アドレスを 12 bit シフトして PPN フィールドへ。今は ASID 未使用 (= 0)。
- PLIC のサイズは `0x40_0000` (4 MiB)。レジスタ実体は数 KiB で済むが、context 軸 × IRQ 軸の 2 次元に広がるためアドレス空間としては大きい。`0x10000` で張ると claim/complete (PLIC + 0x20_1004) に届かず page fault する。
- カーネル identity map では U bit を立てない。U bit は leaf PTE 専用で、中間 PTE では意味を持たない (= `Pte::new_table` でも立てない)。

### 次にやること

- ~~**(c'-2) `push_off` / `pop_off` + 自前 Spinlock の導入**~~ — **完了** (D0015)。
- ~~**(c'-3) PLIC + UART RX**~~ — **完了**。priority/threshold/enable/claim/complete のサイクル + UART IER + sie.SEIE + dispatch (code=9) を通し、キー入力を割り込み駆動で受け取れるようになった。
- ~~**(e) 物理ページアロケータ**~~ — **完了** (D0016, D0017, D0018)。`[__kernel_end, PHYSTOP)` を 4 KiB freelist で管理。`PhysAddr` / `VirtAddr` newtype 導入とグローバル割り込み有効化の整理も同時に実施。
- ~~**(f) Sv39 ページングを identity map で有効化**~~ — **完了** (D0019, D0020, D0021)。`Pte` / `PageTable` / `walk` / `mappages` / `kvmmake` / `kvminithart` を実装。W^X 分離、A/D 強制 OR、CSR ラッパを cpu.rs に集約。
- **(g) ユーザページテーブル + 最初の U-mode 遷移**: init 用の PT を別途構築し、`sstatus.SPP = 0` / `sepc = entry` を設定して `sret` で U-mode へ。これに先立ち `Pte::new_leaf` の flags に `PTE_U` を含める経路を整理する。次に取り組むタスク。

### 参照

- RISC-V SBI Specification — Timer Extension (EID = `0x5449_4D45` "TIME", FID = 0)。Legacy Timer Extension (EID = `0x00`) も等価機能。
- RISC-V Privileged Spec — `sie` / `sip` / Supervisor Timer Interrupt の番号付け (Table 4.2: interrupt code 5)。
- xv6-riscv `kernel/trap.c::devintr()` (interrupt 種別の振り分け)、`kernel/start.c::timerinit()` (M-mode で mtimecmp を直接設定する版、参考)。
- xv6-riscv `kernel/spinlock.c` (`acquire` / `release` / `push_off` / `pop_off`)、`kernel/proc.h` (`struct cpu`)。
- Rust std — `std::sync::Mutex` の `Sync` 境界 (`unsafe impl<T: Send> Sync`)、`MutexGuard` の Deref/DerefMut/Drop パターン。
- Rust Reference — `UnsafeCell`、`addr_of_mut!`、`static mut` の 2024 edition での扱い。
- RISC-V PLIC Specification (公式) — Priority / Pending / Enable / Threshold / Claim/Complete の定義、context の概念。
- xv6-riscv `kernel/plic.c` (PLIC 初期化と claim/complete のサイクル)、`kernel/trap.c::devintr()` の external 経路、`kernel/uart.c::uartintr()`。
- QEMU `hw/intc/sifive_plic.c` (PLIC のエミュレーション実装、context マッピングの確認用)。
- xv6-riscv `kernel/kalloc.c` (`kfree` / `kalloc` / `freerange` / `kinit`)、`kernel/memlayout.h` (アドレス定数の置き場)。
- Rust Reference — `unsafe impl Send` の使いどころ、`Copy` 型のメソッドレシーバ慣習。
- Rust nomicon — 内部可変性 / `*mut T` の Send/Sync。
- RISC-V Privileged Spec — Sv39 PTE フォーマット (§4.4.1)、`satp` レイアウト (§4.1.11)、`sfence.vma` (§4.2.1)、A / D bit の更新方式 (Svade / Svadu)。
- xv6-riscv `kernel/vm.c` (`walk` / `mappages` / `kvmmake` / `kvminithart`)、`kernel/riscv.h` (PTE_V / PTE_R / PTE_W / PTE_X / PTE_U / `MAKE_SATP` などの定数とマクロ)。
- xv6-riscv `kernel/kernel.ld` (W^X 区切りの linker 例)。

---

## 2026-05-02

### やったこと

- (g-1) user PT 作成 + 1 ページ map + walk 確認まで完了。`vm::uvmcreate -> *mut PageTable` (kalloc + zero) と `vm::uvmfirst(&mut PageTable, &[u8])` (kalloc + memcpy + `mappages` with `PTE_R|W|X|U`) を追加。INITCODE は `ecall` の機械語 4 バイト (`73 00 00 00`) を直書き。
- `kmain` で `walk(&mut *pt, VirtAddr(0), false)` の結果をダンプし、以下を観測: `pte = 0x21fee0df` (下位 8 bit = `0xdf` = `V|R|W|X|U|A|D`、G=0)、`pa = 0x87fb8000` (= `[__kernel_end, PHYSTOP)` 内)、`payload = 73 00 00 00` (read back)。leaf PTE が U bit 付きで作られ、kalloc 由来の物理ページに INITCODE が書き込まれていることを確認。
- (g-2-a) 設計判断を 3 件確定 (D0022 / D0023 / D0024)。コードはまだ書かない。
  - D0022: user PT は xv6 流の raw 関数群 (`uvmcreate -> *mut PageTable` 等) で扱い、所有・解放は呼び出し側 (将来の `Process` 構造体) に持たせる。
  - D0023: U/S 切替は xv6 流のトランポリン方式。`MAXVA = 1<<38`、`TRAMPOLINE = MAXVA - PGSIZE`、`TRAPFRAME = MAXVA - 2*PGSIZE`。stvec は走行 mode で切替、`usertrap` と `kerneltrap` の経路を完全分離。
  - D0024: 最小 `Process { pagetable, trapframe, sz }` を (g-2-c) で導入。`static` 1 個固定の置き方はせず、後で `[Process; NPROC]` に拡張できる形にする。
- (g-2-b) 完了。`src/asm/trampoline.S` を新規作成 (まずは `.zero 4096` のスタブ)、`linker.ld` に `.text.trampoline` セクションと `__trampoline_start` / `__trampoline_end` symbol を追加、`memlayout.rs` に `MAXVA = 1<<38` / `TRAMPOLINE` / `TRAPFRAME` 定数 + `trampoline_start()` アクセサを追加。`vm::kvmmake` の末尾で kernel PT に `TRAMPOLINE` を貼り、`kmain` から user PT 側にも同じ物理ページを貼って両 PT で walk して同一 PA (`0x80207000`) が見えることを確認。エディタ的に `.S` ファイルを切り出すために `src/asm/entry.S` も新規作成し、`main.rs` の `global_asm!` を `include_str!("asm/entry.S")` 経由に統一。
- (g-2-c) 完了。`Trapframe` 構造体 (xv6 と byte-for-byte 同一の 36 フィールド、`#[repr(C)]` + size assert) を `src/proc.rs` に追加。`Process { pagetable, trapframe, sz }` を新設、`Process::new` で trapframe ページを kalloc + ゼロクリア + user PT の `TRAPFRAME` に PTE_R|W (no U) で借用マップ。`vm::proc_pagetable(trapframe)` ヘルパに uvmcreate + trampoline マップ + trapframe マップを集約。動作確認: trapframe の walk PTE が `0xc7` (V|R|W|A|D, no U/X)、PA = `Process::trapframe`、`(*p.trapframe).kernel_sp = 0xdeadbeefcafebabe` を書いて `*(p.trapframe as *const u64).offset(1)` で読み戻して field offset 8 を実証。
- (g-3-a) 完了。`Process` に `kstack: usize` 追加 (= xv6 流に "底" を保存。kalloc 1 ページの PA そのまま、識別マップに乗っているので追加 mapping 不要、D0025)。`Cpu` に `proc: *mut Process` フィールド追加。`proc::myproc()` 実装。`trap.rs` に `usertrap` / `usertrapret` の skeleton (`unimplemented!()`) を配置して、ビルドが通り出力が変わらないことを確認。
- (g-3-b) 完了。`src/asm/trampoline.S` の `.zero 4096` を `uservec` / `userret` の本体に置き換え。31 GP regs の sd / ld ペア (a0 は sscratch 経由)、satp 切替の前後の `sfence.vma` ペア、`csrrw a0, sscratch, a0` による a0 ↔ sscratch スワップ。引数規約は xv6 master と同じ 2 引数版 (`userret(TRAPFRAME, satp)`、a0 = TRAPFRAME ポインタ、a1 = user satp 値) を採用。
- (g-3-c) 完了。CSR ラッパ群 (`r_sepc` / `w_sepc` / `r_scause` / `r_sstatus` / `w_sstatus` / `w_stvec` / `r_tp` + `SSTATUS_SPP` / `SSTATUS_SPIE` 定数) を `cpu.rs` に追加 (D0021 の方針通り)。`memlayout.rs` に `trampoline_uservec_va()` / `trampoline_userret_va()` (= `TRAMPOLINE + (sym - __trampoline_start)`) を追加。`usertrap` 本体 (= stvec を kernelvec に戻す + sepc を trapframe.epc に退避 + scause=8 で print + loop) と `usertrapret` 本体 (= intr_off + stvec 切替 + trapframe の kernel 側 5 値セット + sstatus 整備 + sepc セット + `userret(TRAPFRAME, satp)` を transmute 越しに call) を実装。`kmain` で Process を作り `cpu.proc` にセット、`p.sz = PGSIZE` / `p.trapframe.epc = 0` / `p.trapframe.sp = PGSIZE` を初期化、`trap::usertrapret()` で初降下。観測: `usertrap: U-mode ecall, epc = 0x0` が出力され、kmain → usertrapret → userret → sret → U-mode → ecall → uservec → usertrap の **6 段全経路** が通った。`tick N` も並走。

### 詰まったこと / わかったこと

- user PT の所有モデルとして Rust の所有型 (`UserPagetable` newtype + `Drop`) を立てるかどうか検討。再帰的な PT 走査で解放する Drop 自体は Rust に綺麗に乗る (`Box<Tree>` などと同じ) が、本質的な論点は **leaf データページの「所有 vs 借用」をどこに持つか**。
- 区別の置き場所の選択肢: (i) アドレス範囲 (xv6 size-based)、(ii) PTE の RSW bit、(iii) `Vec<OwnedPage>` の台帳、(iv) PTE_U bit を構造的ディスクリミネータに使う。
- (iii) は kernel heap が要り今は使えない。(ii) は仕様外の用法で抵抗感がある。(iv) は xv6 のアドレス空間モデルと「PTE_U=1 ⇔ user 所有」が偶然一致しているので構造的に使えるが、user 共有メモリ (mmap `MAP_SHARED` 等) が来た瞬間に崩れる前提付き。
- 結局、Rust の型システムは **per-page の状態を動的なページ数で追えない** (storage が runtime に落ちる) ので、API の入口で型を分けても storage の表現は同じ問題を持つ。xv6 流に raw 関数群 + 所有は呼び出し側、と素直に書くほうが小さく済むという結論 (D0022)。
- `&'static mut PageTable` は kernel PT (= 唯一・永続) には妥当だが、user PT には不向き (`uvmfree` を入れた瞬間に 'static 仮定が崩れる)。`uvmcreate` の戻り値は `*mut PageTable`。
- xv6 のトランポリン方式は単純に「satp 切替の前後の数命令が両方の PT で同じ VA に見える必要がある」から導かれる。`csrw satp` の前後で PC を維持するには、その PC が新旧両方の PT に同一 VA で存在しなければいけない。同一物理ページを kernel PT・user PT の両方の `MAXVA - PGSIZE` にマップする、というのがその実体。
- xv6 の trapframe に `kernel_satp` / `kernel_sp` / `kernel_trap` / `kernel_hartid` が含まれているのは SMP のためではなく **satp 境界そのもの** が理由。`uservec` は S-mode だが `satp = user` で走るので、user PT に見えるもの (= trampoline + trapframe + user ページ) しかアクセスできない。kernel 側のグローバル状態・スタック・関数アドレスはどれも取りに行けないので、`usertrapret` がこの 4 値を **直前に trapframe に書き込んでおく**。SMP はこれを per-hart 値に拡張する話で、シングルコアでもフィールド自体は必要。
- `MAXVA` を `1 << 39` ではなく `1 << 38` に丸めるのは Sv39 の sign extension 制約。HW は `VA[63:39]` の sign extension を厳密にチェックするので、上限直前 (= 上半分との境界) を踏むと canonical 違反のリスクがある。xv6 と同じく lower half の半分だけ使う。
- `walk(&mut *pt, ..., false)` のように呼び出し側で `*mut PageTable` を `&mut PageTable` に deref する形は、xv6 の `pagetable_t` (= ポインタ) を Rust の関数引数に変換する境界として妥当。`mappages` / `walk` 自体は引き続き `&mut PageTable` を取る。
- linker.ld の section ordering バグ。元の `*(.text .text.*)` の `.text.*` ワイルドカードが `.text.trampoline` も貪欲に吸ってしまい、その後の `KEEP(*(.text.trampoline))` には何も残らず `__trampoline_start` だけが宙に浮いた位置に置かれていた。`.zero 4096` だった (g-2-b) では中身に意味がなく、PA 一致テストも「対応していない物理ページ同士が偶然一致」する形で通っていた (= テスト設計の甘さ)。`uservec` / `userret` の実コードが入った (g-3-b) で初めて症状が顕在化し、`addr_of!(uservec) as usize - trampoline_start()` が underflow する panic で気づいた。修正は `KEEP(*(.text.trampoline))` を `*(.text .text.*)` の前に置いて先取りさせる順序入れ替え。ld の first-match-wins セマンティクス通り。
- 切り分けの決め手は `nm | grep` で `__trampoline_start <= uservec < userret < __trampoline_end` を確認すること。(g-2-b) の段階で symbol 位置まで検証していれば早期発見できた (= 反省点として「セクションを切ったときは中身がスタブでも nm でアドレス位置を確認」をルール化したい)。
- `cargo run` を直接打つと runner 未設定なのでホスト (macOS) が RISC-V ELF を execve しようとして "cannot execute binary file" になる。D0007 で Makefile に統一しているので必ず `make run` を経由する。
- `linker.ld` を変えても `cargo build` がリンクをスキップすることがある (linker.ld は Rust source の依存に乗らない)。挙動が変わらないときは `cargo clean` を試す。
- TRAMPOLINE 経由の関数呼び出しに `transmute` が必要な理由: `extern "C" { fn userret(...); }` で宣言してリンカに symbol アドレスで呼ばせると、その symbol の物理アドレス (= `[KERNBASE, __etext)` 内の識別マップで届く位置) で関数が始まる。`csrw satp, a1` の瞬間に user PT に切り替わると、user PT には `[KERNBASE, __etext)` のマッピングが無いため、次の命令フェッチで page fault する。**TRAMPOLINE VA 越しに呼ぶと kernel PT・user PT 両方で同じ VA がマップされている** ので、satp 切替を跨いで PC が連続する。stvec の値も同じ理由で TRAMPOLINE 経由のアドレスにする必要がある。
- sscratch の TRAPFRAME 規約: U-mode で走っている間は `sscratch = TRAPFRAME` を保つというプロトコル。`csrrw a0, sscratch, a0` 1 命令でこの不変条件を維持する。userret 末尾と uservec 冒頭でこの swap が対称に出てくる。
- sepc は **同期 trap (ecall, page fault, illegal instr 等) では trap を起こした命令そのもののアドレス**、**非同期 trap (timer, external 割り込み) では中断された命令のアドレス**。ecall を sret でそのまま戻すと同じ ecall を再実行 = 無限ループになるので、syscall ハンドラは `p.trapframe.epc += 4` で次に進める必要がある (今回は loop で停止するので関係なし、(h) で実装)。
- カーネル自身は xv6 では「プロセス」として管理されない。実行スレッドは「(1) どれかの user proc が trap してカーネル側で走っている (= proc の kstack を借りる)」「(2) per-CPU の scheduler スレッド」の 2 つのみ。Linux も基本同じだが、kthread を `task_struct` として持つ点で拡張されている (idle = swapper も task)。`Process` に `kstack` / `context` を持たせる構造はこれを反映していて、scheduler / fork が来る段階で `swtch` と一緒に意味が付く。
- `userret` の引数を 2 引数版 (`userret(TRAPFRAME, satp)`) にした理由: 1 引数版だと TRAPFRAME を asm 内で `li` するか sscratch から取り出す必要があり、初期化経路の手間が増える。a0 = TRAPFRAME ポインタを最後まで使い続けて a1 = satp 値を `csrw satp, a1` で消費する形が一番素直。
- usertrap 冒頭で `w_stvec(kernelvec)` を必ず行う理由: usertrap 中に kernel 内 trap (= page fault 等) が発生したとき、stvec が uservec のままだと TRAMPOLINE 経由で再入してしまう。kernel 内 trap は kernelvec (= 既存 `trap_entry`) で受ける必要がある。
- kstack を kernel PT の高位 VA + ガードページに置かず、識別マップそのまま使う方針 (D0025)。スタックオーバフローはサイレントなメモリ破壊になりうるが、現段階ではマッピング操作 0 で済む単純さを取る。

### 次にやること

- ~~(g-2-b) trampoline section + マッピング~~ — **完了**。
- ~~(g-2-c) Trapframe + Process 構造体~~ — **完了**。
- ~~(g-3) 最初の sret + usertrap 最低限~~ — **完了**。

進める順:

- **(h) syscall ABI の雛形**: `usertrap` の `scause = 8` 経路を `loop {}` から `syscall()` 呼び出しに置き換え、`a7 = syscall 番号 / a0..a5 = 引数 / 戻り値は a0` の慣例で `write` (= UART 経由 print) と `exit` (= panic / loop) を最初に通す。INITCODE を `ecall` 1 命令から数命令に拡張して syscall を 2 種実行できることを観測。`epc += 4` を syscall 復帰経路に入れて user に戻す経路 (= 図 A→B→C のサイクル) も初めて実走する。
- (i) init を埋め込み ELF としてロード: `include_bytes!` で取り込んだ ELF をパースし、ユーザ PT にマップ。エントリポイントへ sret。INITCODE 直書き経路はここで縮退。
- スケジューラ・fork は (i) が動いてから (D0009)。

### 参照

- xv6-riscv `kernel/exec.c::uvmfirst`、`kernel/vm.c::uvmcreate`。
- xv6-riscv `kernel/proc.h::struct trapframe` / `kernel/trampoline.S` / `kernel/trap.c::usertrap` / `kernel/trap.c::usertrapret`。
- xv6-riscv `kernel/memlayout.h` の `MAXVA` / `TRAMPOLINE` / `TRAPFRAME` 定義。
- RISC-V Privileged Spec — Sv39 の VA レイアウト・canonical 制約 (§4.4)、`stvec` (§4.1.2)、`sstatus.SPP` (§4.1.1)。
- xv6-riscv `kernel/trampoline.S` (uservec / userret の参考実装、レジスタ並び順とオフセット表)、`kernel/trap.c::usertrap` / `kernel/trap.c::usertrapret` (trapframe の kernel 側 5 値の埋め方と stvec 切替の段取り)。
- xv6-riscv `kernel/proc.h::struct proc` (Process 構造体の最終形、`kstack` / `context` などの拡張先)、`kernel/proc.c::allocproc` / `kernel/proc.c::proc_pagetable` (生成経路)。
- ld マニュアル — Section Placement の first-match-wins と SECTIONS のパターン記述順序の意味 (今回の linker.ld バグの根拠)。

---

## 2026-05-03

### やったこと

- (h) syscall ABI の雛形を実装。Linux RISC-V generic 番号 + Linux errno + POSIX semantics 方針を D0027 として記録。
- (h-1) INITCODE を `[u8; 4]` 直書きから `.rodata.initcode` セクション + `__initcode_start` / `__initcode_end` symbol 経由に置き換え。`memlayout::initcode() -> &'static [u8]` で取り出す形に統一。`src/asm/initcode.S` を新設、linker.ld に section と symbol を追加。
- (h-2) `src/syscall.rs` を新設し dispatcher `pub fn syscall()` を配置。`SYS_EXIT = 93`、`SYS_PUTC = 1024` (学習用、Linux 予約域から外した高番号)。`usertrap` の `scause = 8` 経路を `loop {}` から `syscall::syscall()` 呼び出しに置換。`unknown syscall 93` の出力を観測して dispatcher 配線を確認。
- (h-3) `sys_exit` を実装。`println!` で報告 → `cpu::intr_on()` で `sstatus.SIE = 1` → `loop { wfi }`。`[kernel] proc exited with code 0` の後にタイマ割り込みで `tick N` が継続することを確認。
- (h-4) `sys_putc` を実装、INITCODE を `li a0, 'A'; li a7, 1024; ecall; li a0, 0; li a7, 93; ecall; 1: j 1b` に拡張。`A[kernel] proc exited with code 0` の順で出力され、**U → S → U → S → U の完全 1 周回** が初めて通った。
- `usertrap` の戻り値型を `-> !` のまま維持し、末尾で `usertrapret() -> !` を呼ぶ形に整理。型レベルで「U-mode に戻るのが唯一の出口」を表現。

### 詰まったこと / わかったこと

- S-mode trap が起きるとハードウェアが `sstatus.SPIE ← SIE`、`SIE ← 0` を自動でやる。`sret` で逆向きに復元する。**trap ハンドラ実行中は割り込み禁止**という設計。kmain 末尾の `intr_on()` は U-mode 入りのときに SPIE 経由で持ち込まれるが、ecall で S-mode に戻った瞬間に `SIE = 0` にリセットされる。
- 上の帰結として、`sys_exit` 内の `loop { wfi }` に入る前に `intr_on()` が必須。`wfi` 自体は SIE=0 でも保留中の割り込みで復帰するが、**ハンドラへのディスパッチが起きない** = 単に wfi の次の命令に進むだけになり、結果として「タイマ割り込みは起きているのにハンドラが呼ばれない」現象になる。`tick` が出ないのはこれが原因。
- INITCODE の `epc` オフセットは圧縮命令展開を反映する。`li a7, 93` は即値 93 が `c.li` の 6 bit signed 範囲 (-32..31) を超えるので圧縮されず 4 byte (`addi a7, x0, 93`)。`li a0, 0` は範囲内なので `c.li a0, 0` で 2 byte。よって `ecall` のオフセットは `0 + 4 + 2 = 0x6` になる。観測値の `epc = 0x6` はこの計算通り。
- `ecall` 命令は **圧縮形を持たない**ので常に 4 byte。`tf.epc += 4` は固定値で問題ない。一般の同期 trap (illegal instr 等) を syscall と同じ流儀で扱うときは命令長を見て分岐が必要になる場面が出るが、ecall に限れば不要。
- `#[macro_export]` 付きで定義されたマクロは **どのモジュールに書かれていてもクレートルート** に置かれる。他モジュールから使うには `use crate::println;` が Rust 2018+ の流儀。エラーメッセージが提案する `#[macro_use]` は古いスタイル。
- `usertrap` の戻り値型は `-> !` のまま末尾で `usertrapret() -> !` を呼ぶのが綺麗。`()` でもコンパイルは通るが、「U-mode に戻るのが唯一の出口」という意図が型レベルで表現できなくなる。
- syscall の戻り値を `i64` で持ち、`tf.a0 = ret as u64` で trapframe に書き戻す。`-errno` を負値で表現するための符号付き型。`tf.a0` を `u64` で持っているのは trapframe の他の汎用レジスタフィールドと型を揃えるため。
- `epc += 4` の置き場は `match` の **後** が綺麗。SYS_EXIT 経路は loop で帰らないのでこの加算は実行されず、SYS_PUTC 経路は通って次の命令に進む、という自然な分岐になる。
- INITCODE のサイズデバッグ手段として `println!("initcode len = {}", initcode().len())` を kmain で出すのが便利。`KEEP` 漏れだと len = 0、section ordering バグだと len は 0 でないが nm で位置がずれる、の切り分けが効く。

### 次にやること

- ~~(h) syscall ABI の雛形~~ — **完了** (D0027)。
- **(i) init を埋め込み ELF としてロード**: D0026 で決めた `user/` ディレクトリ + multi-bin Rust crate を立ち上げる。`user/Cargo.toml` + `user/src/lib.rs` (= `_start` / panic_handler / syscall stub) + `user/src/bin/init.rs` の 3 ファイル構成。Makefile に user 側ビルドを追加し、`include_bytes!` で取り込んだ ELF をパースしてユーザ PT にマップ、エントリへ `sret`。INITCODE 直書き経路はここで縮退。
- syscall stub をどう書くか (= `ecall` を Rust 関数として包む) と、kernel-user 間の syscall 番号定数の共有方法 (D0026 の保留分) を (i) 着手時に決定。
- `write(fd, buf, len)` の `copyin` 実装も (i) のスコープ。POSIX semantics に準拠して fd=1 のみ UART 出力、それ以外は `-EBADF`。

### 参照

- RISC-V Privileged Spec — `sstatus.SIE` / `SPIE` の trap 自動切替 (§4.1.1)、`wfi` 命令 (§4.6)、`sret` の動作。
- RISC-V Unprivileged Spec — 圧縮命令 (`c.li` の即値範囲、`ecall` に圧縮形がないこと)。
- Linux generic syscall 表 — `linux/include/uapi/asm-generic/unistd.h` (`__NR_write = 64`, `__NR_exit = 93` 等の RISC-V/ARM64 共通番号体系)。
- xv6-riscv `kernel/syscall.c` (dispatcher の参考)、`user/initcode.S` (INITCODE の参考)、`user/usys.pl` (syscall stub 自動生成、(i) で user crate を立ち上げる際の参考)。

---

## 2026-05-04

### やったこと

- (i-1) user crate skeleton を立ち上げ。`user/Cargo.toml` (`panic = "abort"` のみ、`[[bin]]` は書かず autodiscovery に乗る) + `user/.cargo/config.toml` (target + `-Tlinker.ld`) + `user/linker.ld` (`. = 0;` から `.text` / `.rodata` / `.data` / `.bss`、`.rodata` 以降は `ALIGN(4096)` で W^X 余地を残す) + `user/src/lib.rs` (`SYS_EXIT` / `SYS_PUTC` 番号定数 + `syscall6` inline asm + `exit` / `putc` ラッパ + `panic_handler`) + `user/src/bin/init.rs` (`#[unsafe(no_mangle)] pub extern "C" fn _start() -> !` で `putc(b'A'); exit(0)`)。
- (i-2) `src/exec.rs` を新設し ELF64 ローダを自前実装。`Ehdr` / `Phdr` を `#[repr(C)]` で定義し、`include_bytes!` 由来の align 1 buffer から `core::ptr::read_unaligned` で取り出す。検証 6 点 (magic / class / data / machine / type / phentsize) + 長さ + phdr テーブル境界。PT_LOAD ごとに `load_segment` で per-page kalloc_zeroed → 部分 memcpy → mappages、`p_flags` を PTE_R/W/X にマップ。`p_filesz < p_memsz` の bss 部は `kalloc_zeroed` のおかげで自動ゼロ。最後に最終 PT_LOAD 直上 1 ページを user stack として alloc + map (PTE_U|R|W) し、`(entry, sp, sz)` を返す。
- 既存の `kalloc()` (junk fill 0x05 を残す) はそのまま、`kalloc_zeroed()` を新設し vm.rs の walk / kvmmake / uvmcreate、proc.rs の trapframe を置換。kstack は raw `kalloc()` のまま (即 push で上書きされるためゼロ化が無駄)。
- (i-3) INITCODE 直書き経路を撤去。`src/asm/initcode.S` を削除、linker.ld の `.rodata.initcode` セクションと `__initcode_start` / `__initcode_end` symbol、memlayout.rs の `initcode()`、vm.rs の `uvmfirst` をすべて削除。kmain は `exec::exec(&mut *p.pagetable, exec::INIT_ELF)` の戻り値で `tf.epc` / `tf.sp` / `p.sz` を埋める形に一本化。`init.rs` 側の出力を `'A'` から `'B'` に変えて新ローダ経由の ELF が走っていることを `B[kernel] proc exited with code 0` で観測。
- Makefile を user 連動に整備。`USER_ELF` を定数化、`user:` で `cd user && cargo build --release`、`build:` を `user:` に依存させる、`clean:` に `cd user && cargo clean` を追加して 2 つの target ディレクトリを両方掃除。`.gitignore` に `user/target` を追加。
- `.vscode/settings.json` を新設。`rust-analyzer.linkedProjects` に kernel / user の両 Cargo.toml を独立 project として登録、`check.allTargets` / `cargo.allTargets` を false にして no_std crate で test target が拾われる挙動を抑止。
- (i-4) `sys_write` を POSIX semantics で実装 (D0028)。kernel 側に `pub enum CopyError { Fault }` + `copyin(pt, dst: &mut [u8], src_va: VirtAddr)` を `vm.rs` に追加。`walk` ベースで page 跨ぎ対応、各 page で `MAXVA bound` / `PTE_V (leaf)` / `PTE_U` の 3 点 check を inline で展開。`syscall.rs` に `EBADF = 9` / `EFAULT = 14` 定数 + `errno_of_copy(CopyError) -> i64` + `sys_write` ハンドラ。`fd == 1 || fd == 2` を console に通し、それ以外は `-EBADF`、`len == 0` は `0` を即返。kernel stack 上の `[u8; 128]` バッファでチャンク (per-call kalloc は採らない)。
- 既存の `console::CONSOLE` (`Spinlock<Uart16550>`) を経由する形にして、kernel `println!` と user `write` の出力交錯を防ぐ。`console.rs` に `pub fn write_bytes(&[u8])` を追加し、`sys_write` はチャンク 1 個ごとに lock 取得 → 解放 (= xv6 `consolewrite` 流の粒度)。
- `user/src/lib.rs` に `pub fn write(fd: i32, buf: &[u8]) -> isize` を追加 (safe wrapper、内部は `syscall6`)。`SYS_PUTC = 1024` と user 側 `putc` / kernel 側 `sys_putc` を削除。`SYS_WRITE = 64` (Linux generic) で確定。
- `init.rs` を `write(1, b"Hello, world!\n")` に書き換え、戻り値が負なら `exit(1)`。`Hello, world!\n[kernel] proc exited with code 0` を観測。

### 詰まったこと / わかったこと

- `include_bytes!` の戻り値型は `&'static [u8; N]` でアライメントが 1。中に u64 を持つ `Ehdr` (`align_of` = 8) を `*` deref で読むと **言語仕様上 UB** (= ハードウェアが misaligned を許すかどうか以前の話)。`core::ptr::read_unaligned` は内部で memcpy → 通常 load に展開してくれるので常に sound。`Phdr` も同事情でループ内 `read_unaligned`。逃げ道としては `#[repr(align(8))] struct Aligned<T: ?Sized>(T)` でラップして取り出す方法もある。
- `<[u8]>::as_ptr()` はターボフィッシュを取らず常に `*const u8`。`elf[off..].as_ptr::<Phdr>()` はコンパイルエラー、`elf.as_ptr().add(off) as *const Phdr` で書く。
- ELF magic の比較は `ehdr.e_ident[..4] == ELF_MAGIC` でよい。slice `[u8]` と配列 `[u8; 4]` の `PartialEq` impl が std にあり、`==` 内で auto-ref が効く。`&` を中途半端に付けると型が噛み合わない。
- Edition 2024 では `unsafe fn` の中でも `unsafe_op_in_unsafe_fn` lint が有効なので、`asm!` を呼ぶには **明示的な `unsafe { ... }` ブロック** が必要。`#[no_mangle]` も `#[unsafe(no_mangle)]` 形式が新しい (RFC 3552)。
- inline asm の `inlateout("a0") a0 => ret` は a0 が「入力かつ出力」であることを伝える宣言。`inout` だと入力読み終え前に出力に書き始める可能性が compiler の解釈に残るため、`lateout` で「全 input を読み終えてから out」を保証する。
- `clobber_abi("C")` は caller-saved (a0–a7, t0–t6, ra, ft0–ft11, fa0–fa7) を全部 clobber 扱いにする宣言。我々の kernel は trapframe で全 GP を保存するので実害はないが、将来 caller-saved の保存だけに最適化したときに user 側が壊れないよう defensive に付ける。
- `options(nomem)` は付けない。将来 `write` で user buffer を読むときに「buf に書いた値はまだ memory に flush されていない」と compiler が判断してマージしてしまう事故を避けるため。`options(nostack)` だけ付ける。
- `kalloc()` は xv6 流の junk fill (0x05) を残しつつ、`kalloc_zeroed()` を別 API として並べる構成にした。junk fill は uninit 読みを目立たせる防御線として有効、ゼロ要求が多い場面 (PT 中間ノード / trapframe / ELF segment / bss) は `kalloc_zeroed` 1 行で済む。Linux の `__GFP_ZERO` 相当。
- ELF レイアウトの典型: `.text` のみの最小 init では PT_LOAD は 1 個 (R+E)、`.rodata` / `.data` / `.bss` が空なら segment は省略される。`GNU_STACK` (空 segment、stack non-exec ヒント) と `RISCV_ATTRIBUTES` は PT_LOAD 外なので、ローダ側で **PT_LOAD 以外を無視する**設計なら触らずに済む。
- ELF の section と segment は別概念。section は linker が編集する単位、segment (PT_LOAD) はローダが見る単位。`p_offset` (file 内オフセット) と `p_vaddr` (実行時 VA) は別個に決まり、`p_offset == p_vaddr (mod p_align)` の制約だけある。我々の linker.ld の `. = 0;` で `p_vaddr = 0`、ld が file 内では `p_offset = 0x1000` に置く (page align のため) という形。
- user stack は最終 PT_LOAD の直上 1 ページに置く xv6 流。`init` の場合 `.text` 終端 0x2c → round_up で 0x1000、0x1000–0x2000 が stack ページ、`sp = 0x2000` (top, grows down)。今後 init 以外を入れたとき .data / .bss が伸びると stack の VA も自動で押し上がる。
- D0026 で保留にしていた「syscall 番号の kernel/user 共有方法」は **両側に同じ const を二重持ち + コメントで相互参照** で決着。番号が増えてきたら common crate 化を再検討。
- `cargo clean` は manifest を見つけた crate の target/ しか触らない。ルートで打っても `user/target/` は残るので、Makefile の clean に `cd user && cargo clean` を足す必要がある。
- rust-analyzer のデフォルトは `cargo check --all-targets` 相当を走らせる挙動で、no_std crate の test/bench target で std が要求されて警告になる。`rust-analyzer.check.allTargets = false` でこのフラグを外せば test/bench が check 対象から外れ、lib + bins は通常通り live check される (= check 自体を切るわけではない)。
- workspace に組み込まない複数 Cargo project を 1 リポジトリ内で同時に解析させるには `rust-analyzer.linkedProjects` に各 Cargo.toml を列挙する。これで「このファイルはどの crate にも属していない」警告が消える。VSCode が読むのは `.vscode/settings.json` (複数形)、`setting.json` のタイポは黙って無視されるので設定が効かない原因として常に候補に入る。
- `include_bytes!` の話と同じ事情で、user buffer の VA は kernel から「現在の satp に依らず・page 跨ぎを処理し・不正 VA を fault に落とさず」読む必要がある。これが xv6 でいう `copyin` / `copyinstr` / `copyout` の存在理由。
- `walk` が `Some(*mut Pte)` を返すのは「intermediate を辿り終わった」だけの保証で、leaf 自体の `PTE_V` は別途 check が要る。「`Some` だから valid」ではない。`walk` は intermediate が leaf (= super page) の場合だけ `None` を返し、最終 level に到達したら無条件で leaf entry のポインタを返す仕様。
- VA の `MAXVA` bound check は Sv39 の VPN 抽出が `& 0x1ff` で 9 bit ずつしか取らないこと (= bit 39+ を黙って捨ててエイリアスする) が理由。`walk` は中で check していないので copyin/copyout 側の責務。
- `PTE_U` check が無いと user が `buf = TRAMPOLINE` / `buf = TRAPFRAME` を渡して kernel メモリを syscall 戻り値経由で抜き出せる。security defense として必須。`copyout` 方向だと書き換えも可能になるのでさらに重要。
- `&[u8]::as_ptr()` は **空 slice でも non-null dangling を返す Rust 保証** がある。kernel 側で `len == 0` 早期 return すれば touch せず安全 (= dangling ptr を walk しない)。
- `console::CONSOLE` が既に `Spinlock<Uart16550>` なので、`sys_write` から `uart::putc` 直叩きすると lock を素通りして kernel `println!` と byte 単位で交錯する。`console::write_bytes(&[u8])` を 1 個足して経由させるのが最小コスト。
- `sys_write` 全体を 1 lock にすると `copyin` を lock 保有中に呼ぶことになり、page fault や将来 sleep で lock を抱えて止まる。chunk per lock で粒度を下げるのが xv6 (`consolewrite`) の作法。POSIX 的にも write の atomicity は (PIPE_BUF 以下の pipe 以外では) 保証されないので問題なし。
- `walk` 戻り値の追加 check 3 点 (MAXVA / PTE_V / PTE_U) は inline で展開した。`copyout` / `copyinstr` が来た時点で `walk_user` (= xv6 の `walkaddr`) として抽出予定。今 1 箇所だけのために抽象を切っても再利用が見えないため。
- syscall stub は `pub fn write(fd: i32, buf: &[u8]) -> isize` を **safe** にした。`&[u8]` を取る形なら `(ptr, len)` が型で揃って渡るので呼び出し側で UB を作れない。kernel 側で walk + PTE_U が二重防御として効くので、user crate 側は安全 API。`isize` を返すのは rv64 で `i64 == isize`、success = byte 数 (≥0)、failure = `-errno` (<0) を 1 値で表す libc `ssize_t` 流儀。
- POSIX `write(2)` は **short write 許容** が API 規約。我々の `sys_write` は UART + chunk loop で全部書き終わってから return するので実際には short write しないが、user 側で「戻り値が要求 byte 数未満かも」と扱うのが筋。`write_all` 相当のループは別関数として後で追加。

### 次にやること

- ~~(i-1) user crate skeleton~~ — **完了**。
- ~~(i-2) ELF ローダを kernel に追加~~ — **完了**。
- ~~(i-3) INITCODE 直書き経路の撤去~~ — **完了**。
- ~~(i-4) `sys_write` を実装~~ — **完了** (D0028)。
- (i 完了) シェル到達への次のマイルストーン:
  - **(j) スケジューラ + fork + exec syscall**: `proc.rs` を `[Process; NPROC]` に拡張、context switch (`swtch.S`)、`scheduler()` / `yield()` を追加。`fork` は親 PT を deep copy (`uvmcopy`)、`exec` は現在 PT を破棄して新 ELF をロード (= 既存 `exec.rs` をそのまま流用できる)。
  - (k) `sys_read` を console から: 行バッファ + キーエコー (cooked mode) を `console.rs` に。`getc` は PLIC + UART RX 割り込み経由。
  - (l) 簡易 FS: まずは RAM FS、その後 xv6 流 inode FS。
  - 順序は (j) → (k) → (l) が素直 (シェルは fork + exec + read を最低限要求する)。
- copyout / copyinstr が必要になった時点で `walk_user` (= xv6 `walkaddr`) を抽出する。今 inline 展開した 3 点 check の重複が出てくるタイミング。

### 参照

- ELF spec (System V ABI) — `Elf64_Ehdr` / `Elf64_Phdr` のフィールド配置、`e_machine = EM_RISCV (243)`、`p_type = PT_LOAD (1)`、`p_flags` の `PF_R / PF_W / PF_X` ビット。
- Rust reference — `core::ptr::read_unaligned` の semantics、`include_bytes!` のアライメント保証 (= 1)、Edition 2024 の `unsafe_op_in_unsafe_fn`、`#[unsafe(no_mangle)]` (RFC 3552)。
- Rust inline asm reference — `inlateout` / `lateout` の意味、`clobber_abi`、`options(nostack)` / `options(nomem)`。
- xv6-riscv `kernel/exec.c` (PT_LOAD ループ + `loadseg` + ustack + sp 設定の参考)、`user/user.ld` (user 側 linker.ld の参考)、`user/usys.pl` (syscall stub の生成例)。
- rust-analyzer manual — `check.allTargets` / `cargo.allTargets` / `linkedProjects` の意味と用途。
- xv6-riscv `kernel/vm.c::copyin` / `copyout` / `walkaddr` (アルゴリズム参考)、`kernel/console.c::consolewrite` (lock 粒度の参考)、`kernel/sysfile.c::sys_write` (fd 振り分けの参考)。
- POSIX.1-2017 §`write` — 戻り値規約、`EBADF` / `EFAULT` / `EINVAL` の意味、short write の許容と pipe/PIPE_BUF の atomicity 規約。
- Linux man-pages `write(2)` — `ssize_t` 戻り値の符号規約、`-errno` 慣例。
- RISC-V Privileged Spec §4.4 — Sv39 の VPN 抽出と canonical address 制約 (= 我々の `MAXVA` bound check の根拠)。

---

## 2026-05-05

### やったこと

- (j-0) scheduler 着手前の process table 方針を整理。`ProcessState` / `Context` / `NPROC` / `static mut PROCS` / `allocproc()` を導入し、`Process::new()` 直呼びから `allocproc()` 経由に移行。
- `userinit()` を追加し、`allocproc()` → `exec::exec(INIT_ELF)` → `trapframe.epc/sp` 設定 → `Runnable` 遷移を `proc.rs` に集約。`kmain` は `userinit()` 後に scheduler へ入る形に変更。
- `src/asm/swtch.S` を追加。`Context { ra, sp, s0..s11 }` と offset を合わせ、scheduler context と process kernel context の切替を実装。
- scheduler を xv6 型に寄せて実装。`RawSpinlock` を導入し、per-process lock を `swtch` 跨ぎで保持する方針にした (D0029)。`forkret()` で初回 process 側が lock を release してから `usertrapret()` へ入る。
- `sched()` / `yield_cpu()` / `exit()` を実装。`sys_exit` は従来の `wfi` loop ではなく、`Zombie` に遷移して scheduler へ戻るように変更。
- `usertrapret()` 冒頭に `cpu.noff == 0` assert を追加し、U-mode へ戻る前に kernel critical section を抜けていることを明示。
- timer interrupt による preemption を実装。U-mode 実行中の timer interrupt は `usertrap()` で `scause` の interrupt bit/code を分解して処理し、`timer::handle()` から `yield_cpu()` を呼ぶ。timer interval は 100ms に変更 (D0030)。
- `kmain` で `userinit()` を 2 回呼び、`init` を長めの busy loop + `write(".\n")` にして round-robin の動作確認を実施。観測: 2 つの user process が `start` 後に `.` を交互に出力し、それぞれ `done` / `exit` まで進む。
- preemption 観測用の `userinit()` 2 回呼び出しを通常の init 1 個に戻した。
- `freeproc` / `proc_freepagetable` / `uvmunmap` / `uvmfree` / `freewalk` を追加し、process が所有する trapframe / user pagetable / kstack を解放できる経路を作った。`uvmunmap` と `freewalk` は kernel 内部の不変条件違反を panic で検出する方針。
- `allocproc()` を per-process lock 前提に整理。`Unused` slot を `p.lock` で保護して探し、成功時は `p.lock` を保持したまま返す契約をコメントで明記。途中確保失敗時は `freeproc()` で巻き戻す。`NEXT_PID` は `static mut` から `AtomicUsize` に変更。
- `walk_user_perm` を `PhysAddr` ではなく検証済み `Pte` を返す形に変更し、`Pte::flags()` を追加。`copyin` は xv6 寄せのため `CopyError` をやめて `Option<()>` を返す形に単純化。
- `uvmcopy()` を実装。親 user page を page 単位で deep copy し、親 PTE の permission を子へ引き継ぐ。途中で `kalloc` / `mappages` が失敗した場合は、既に map した子 page と未 map の確保済み page を解放する。
- syscall ABI 方針を再考し、Linux generic 番号 + `-errno` から、xv6 風 syscall 番号 + 失敗 `-1` に変更 (D0031)。`SYS_FORK = 1`, `SYS_EXIT = 2`, `SYS_WRITE = 16` へ変更し、`user/src/lib.rs` に `fork()` wrapper を追加。
- `proc::fork()` / `sys_fork()` を実装。子 process は `uvmcopy` で address space を複製し、trapframe をコピーしたうえで子側 `a0 = 0` に設定。親には子 pid を返す。`init` で `fork()` し、`parent` / `child` がそれぞれ出力して exit することを確認。

### 詰まったこと / わかったこと

- `Context` は trapframe とは別物。kernel context switch では RISC-V psABI の callee-saved register (`sp`, `s0..s11`) と復帰先の `ra` だけを保存すればよい。`a*` / `t*` は caller-saved として `swtch` 呼び出し側が壊れてよい前提。
- `swtch` をまたぐ Rust の `&mut Process` / `&mut Cpu` は参照モデルとして強すぎる。scheduler / `sched` / `yield_cpu` 周辺は raw pointer 中心にし、`&mut` は短いスコープに閉じる方が正直。
- lock を持ったまま `swtch` するのは一見危ないが、xv6 型 scheduler では `p.state` / `p.context` / kernel stack ownership / `cpu.proc` の不変条件を守るための中核。RAII guard では表現しづらいので、`RawSpinlock` は意図的に non-RAII にした。
- `push_off/pop_off` は CPU-local な `noff/intena` を操作する。`sched()` は `swtch` の前後で `intena` を保存・復元しないと、別 context の lock 操作で `cpu.intena` が混ざる。
- `forkret()` で `p.lock.release()` せずに `usertrapret()` へ入ると、`noff == 1` のまま U-mode へ戻り、software 的には critical section 中なのに U-mode では interrupt enabled という矛盾が起きる。`usertrapret()` の `noff == 0` assert はこの種のバグを捕まえる。
- U-mode 実行中の timer interrupt は `kerneltrap()` ではなく `usertrap()` に来る。`stvec` は U-mode 中に trampoline の `uservec` を指しているため、preemption には `usertrap()` 側の interrupt code 5 処理が必要。
- `timer::handle()` は先に次回 timer を予約してから `yield_cpu()` するのが自然。`yield_cpu()` は scheduler へ戻るので、予約を後回しにすると次 tick の設定が遅れる。
- 現時点の syscall 経路は trap 直後の interrupt-off 状態のまま短く走る。将来、長い syscall / sleep 可能な syscall を入れる段階で、xv6 のように syscall 実行前に `intr_on()` するかを再検討する。
- `allocproc()` の成功時に `p.lock` を保持したまま返す契約は Rust の普通の関数境界としては特殊だが、xv6 型 scheduler と同じく「process を `Runnable` として公開するまで」を 1 つの critical section として扱える。契約が見えなくなると危険なので、コメントと `freeproc()` の `p.lock.holding()` assert で明示する。
- `uvmcopy()` は `walk_user_perm(old, va, 0)` で user leaf であることだけを確認し、`PTE_R` は要求しない。execute-only page など permission に関わらず address space の複製対象にするため。
- `mappages` の `size` は byte 数であり、`uvmcopy` からは `PGSIZE` を渡す必要がある。`1` を渡すと偶然 1 page 分に丸められるが、API の意味として誤り。
- `mappages` 失敗時は、まだ map されていない `dst_pa` を `uvmunmap` では回収できないため、明示的に `kfree(dst_pa)` してから既に map 済みの子 page を rollback する必要がある。
- Linux RISC-V には素朴な `fork` syscall がなく `clone` 系になる。ここで Linux ABI に寄せると flags / child stack / TLS などを早く背負うため、当面は xv6 の syscall ABI に揃えることにした (D0031)。

### 次にやること

- `wait` / zombie 回収を実装する。現状は `exit()` した process が `Zombie` のまま残り、`NPROC` を消費し続ける。
- `exit` code を process に保存する `xstate` 相当の field を追加し、`wait` で親に返せるようにする。
- `getpid` は親子の識別確認に便利なので、`wait` 前後の小さい確認用 syscall として候補。
- その後、`exec` syscall と `sys_read` / console input に進む。

### 参照

- xv6-riscv `kernel/swtch.S`、`kernel/proc.c::scheduler` / `sched` / `yield` / `forkret` / `exit`。
- xv6-riscv `kernel/proc.c::allocproc` / `freeproc` / `fork` / `uvmcopy` / `uvmunmap` / `freewalk`。
- xv6-riscv `kernel/syscall.h` — syscall 番号 (`SYS_fork = 1`, `SYS_exit = 2`, `SYS_write = 16`)。
- xv6-riscv `kernel/spinlock.c::acquire` / `release` / `holding`、`kernel/proc.h::struct context`。
- RISC-V psABI — callee-saved register (`s0..s11`, `sp`) と caller-saved register の区別。
- RISC-V Privileged Spec — trap 時の `sstatus.SIE` / `SPIE` / `SPP` 更新、`scause` の interrupt bit と exception code。

---

## 2026-05-06

### やったこと

- `wait(status_ptr) -> pid` を実装。`Process` に `parent: *mut Process` と `xstate: i32` を追加し、`fork()` で親子関係を設定する形にした。
- `exit(code)` は `xstate` を保存し、自分の子を `init` に `reparent` してから `Zombie` へ遷移するように変更。
- `wait` は `PROCS` を全走査し、自分を parent に持つ `Zombie` 子を見つけたら `xstate` を user memory に書き戻して `freeproc()` する。
- kernel → user のコピー用に `copyout` を追加。`copyin` と対になる関数で、user writable page を検証してから page 跨ぎコピーする。
- 不正 `status_ptr` の場合、`wait` は `-1` を返し zombie 子を回収しない。続く正しい `wait` で回収できることを確認した。
- `getpid` syscall を実装。xv6 と同じ `SYS_GETPID = 11` とし、current process の `pid` を返すだけの小さい syscall とした。
- user 側に `wait(&mut i32)` / `getpid()` wrapper を追加し、`fork()` 前後で parent の pid が変わらず、child は別 pid を持つことを確認した。
- `sleep(chan, lock)` / `wakeup(chan)` を実装し、`wait` を `yield_cpu()` polling から sleep-based に変更。`WAIT_LOCK` を導入して `wait` の「子を確認して寝る」と `exit` の「Zombie 化して親を起こす」の lost wakeup を防ぐ形にした。
- `Console` を `Spinlock<Uart16550>` から `RawSpinlock + UnsafeCell<ConsoleInner>` に組み替え、今後の console input buffer と `sleep(input_chan, console.lock)` に備えた。
- UART RX interrupt の処理を `plic.rs` から `console::intr()` に寄せ、受信 byte を console input buffer に積み、`\r` は `\n` に正規化して echo、改行で `wakeup(input_chan)` する形にした。
- `read(0, buf, len)` syscall を実装。`console::read` で行単位に kernel buffer へ読み、`copyout` で user buffer に返す。`fd != 0` は `-1`、`len == 0` は `0`。
- user 側に `read(fd, &mut [u8])` wrapper を追加し、`init` で `type a line:` → 入力 → `read: ...` の確認を行った。
- `copyinstr` を実装。user memory 上の NUL 終端文字列を kernel buffer に読み込み、NUL を含まない長さを返す形にした。`copyin` と同じく `PTE_R` を要求する方針にした。
- `exec.rs` を `loader.rs` に改名し、責務を「ELF bytes を user page table にロードする loader」に整理した。戻り値は tuple ではなく `LoadedImage { entry, sp, sz }` にした。
- loader の失敗時 cleanup を整理。`load_segment` は自分が map した segment page を rollback し、`load_elf` は既に成功済みの user mapping と stack allocation を cleanup してから `None` を返す形にした。
- `SYS_EXEC = 7` / `sys_exec` / user 側 `exec` wrapper を追加。kernel 側は embedded program table (`loader::PROGRAMS`) から名前で ELF を選ぶ方式にした。
- process image の差し替えは `proc::exec(elf)` に切り出した。新 page table と ELF load が成功するまで旧 address space を保持し、成功後に `p.pagetable` / `p.sz` / `trapframe.epc/sp` を commit して旧 page table を free する。
- `user/src/bin/read_line.rs` を追加し、`init` から `fork` → child で `exec("read_line")` → parent で `wait` する検証に変更した。
- `fork -> exec -> read -> write -> exit -> wait` の経路を確認。観測: child が `read_line` に置き換わり、入力 `abcdef` を `read: abcdef` と出力して exit status 0 を parent が受け取った。
- VM / process / scheduler / lock / trap trampoline / UART / PLIC / timer / console 周辺の重要な契約コメントを整理した。特に `freewalk` は leaf mapping が事前に unmap 済みで、page-table page 自体だけを free する前提を明記した。

### 詰まったこと / わかったこと

- `wait` の標準的な形は `pid_t wait(int *wstatus)`。戻り値は終了した子の pid、終了ステータスは user pointer 経由で返すため、kernel から user VA へ書く `copyout` が必要になる。
- `parent: *mut Process` は slot reuse だけ見ると危険だが、親が `freeproc()` される前に `exit()` で全子を `init` に `reparent` する不変条件を置けば xv6 と同じ形で成立する。
- `RawSpinlock.owner` は同期 primitive 内部の診断用 owner なので `AtomicUsize` の CPU id が自然。一方 `Process.parent` は process table 内の関係そのもので、`p.lock` に守られる通常の process field なので raw pointer で扱う判断にした。
- `wait` で `copyout` が失敗した場合は zombie 子を回収しない方が自然。syscall は失敗 (`-1`) し、呼び出し側は正しい pointer で再度 `wait` できる。
- `getpid` は current process の `pid` を読むだけでよい。`pid` は `allocproc()` 後 `freeproc()` まで変化せず、current process が syscall 実行中に解放されることもないため、現段階では追加の lock は不要。
- `sleep(chan, lock)` は「条件確認」と「sleep 登録」の間に wakeup が挟まる lost wakeup を防ぐため、`p.lock` を取ってから渡された lock を release し、`p.state = Sleeping` / `p.chan = chan` を設定して `sched()` する。
- `sched()` は scheduler context に戻る関数で、xv6 型では `p.lock` を持ったまま `swtch` する必要がある。`p.state` / `p.context` / `cpu.proc` / kernel stack ownership の不変条件が `swtch` を跨ぐため。
- `Console` に `UnsafeCell<ConsoleInner>` を入れると自動では `Sync` にならない。`RawSpinlock` が `ConsoleInner` へのアクセスを守るという不変条件を置いて `unsafe impl Sync for Console` を書く必要がある。
- console input buffer は xv6 風に `r/w/e` の単調増加 index を使う ring buffer とした。実配列アクセスだけ `% INPUT_BUF` し、full 判定は `e - r >= INPUT_BUF`。
- `sys_read` は `console::read` の実読込 byte 数だけ `copyout` して返す必要がある。`len` まで loop して読み切ろうとすると、行入力では 1 行読んだ後に残り byte を待って再 sleep してしまう。
- `copyinstr` は通常の `copyin` と違い、固定長ではなく NUL を見つけるまで読む。user 側から渡す path は `b"read_line\0"` のように明示的に NUL 終端する必要がある。`b"read_line"` のままだと lookup 失敗になる。
- syscall としての `exec` と ELF loader は責務が違う。`loader::load_elf` は page table に ELF image を構築するだけ、`proc::exec` は current process の address space を transaction 的に差し替える、`sys_exec` は path copy と embedded program lookup に限定する形が見通しよい。
- `mappages` は途中で intermediate page table を確保した後に失敗すると、その intermediate table は page table に残る。loader 側の失敗処理は最終的に page table 全体を free することでこのケースを回収する。
- `freewalk` は leaf mapping を free する関数ではなく、leaf がすべて消えた後の page-table page を再帰的に free する関数。leaf が残っていたら caller の unmap 漏れとして panic するのが自然。

### 次にやること

- `fork` / `exec` / `wait` / `read` が通ったので、次は簡易 shell に進む。まずは embedded program table から固定コマンド名を選ぶ形で、1 行入力 → fork → child exec → parent wait の最小ループを作る。
- `exec` の引数は現時点では `path` のみ。`argv` / 環境変数 / user stack への引数配置は shell の最小形が動いた後に検討する。
- console input は現状最小 cooked mode。backspace / Ctrl 系 / EOF / 複数 reader の厳密な公平性は未対応。

### 参照

- xv6-riscv `kernel/proc.c::wait` / `exit` / `reparent`。
- xv6-riscv `kernel/vm.c::copyout`。
- xv6-riscv `kernel/sysproc.c::sys_getpid`。
- xv6-riscv `kernel/proc.c::sleep` / `wakeup`。
- xv6-riscv `kernel/console.c::consoleintr` / `consoleread`。
- xv6-riscv `kernel/exec.c` / `kernel/sysfile.c::sys_exec` / `user/sh.c`。
- xv6-riscv `kernel/vm.c::copyinstr` / `uvmunmap` / `freewalk`。

---

## 2026-05-07

### やったこと

- user mode の不正アクセスなど、syscall 以外の synchronous exception を kernel panic ではなく該当 process の kill として扱うようにした。`stval` を読めるようにし、`scause` / `sepc` / `stval` をログに出してから `exit(-1)` する形にした (D0032)。
- `exec` に `argv` を追加。user 側は C の `char **argv` と同じ thin pointer 配列 + NULL 終端を渡し、kernel 側は旧 address space から `KernelArgs` へコピーしてから新 user stack に積む形にした (D0034)。
- `KernelArgs` / `MAXARG` / `MAXARGLEN` を `proc.rs` に追加。`KernelArgs` は kernel stack に置くには大きいため、`sys_exec` では `kalloc_zeroed()` した 1 page 上に確保し、すべての return path で `kfree()` する形にした。
- `proc::exec(elf, argv)` は新しい page table へ ELF をロードした後、`push_argv` で引数文字列と argv pointer array を user stack に配置するようにした。`trapframe.a0 = argc`, `trapframe.a1 = argv_va`, `trapframe.epc = entry`, `trapframe.sp = sp` を commit する。
- `exec` 成功時に syscall 共通処理が `a0` に return value を書き戻すと、次プログラムの `_start(argc, argv)` の `argc` を破壊してしまうため、`SyscallResult::{Return, Replaced}` を導入した。`exec` 成功時は `Replaced` として `a0` を上書きしない (D0033)。
- user library に `execv_cstr(path, argv)` と `Args` view helper を追加。`Args` は `_start(argc, argv)` が受け取った生 pointer を `get(i) -> Option<&[u8]>` で扱うための薄い view とした。
- `read_line` を `_start(argc, argv)` 形式に変更し、`init` から `fork` → child `execv_cstr("read_line", ["read_line", "test", NULL])` → parent `wait` の経路で argv 付き exec を確認した。
- `src/file.rs` を追加し、global opened file table (`NFILE`) と per-process fd table (`NOFILE`) の土台を入れた。`File` は `refcnt` / `readable` / `writable` / `FileKind` を持ち、kind 固有の field は `FileKind` 側に置く形にした (D0035)。
- 初期 device として console を `FileKind::Device { major: CONSOLE_MAJOR }` で表現した。`file::read` / `file::write` は `FileKind::Device` を console operation に dispatch する。
- `userinit()` で fd 0/1/2 に console device file を割り当てるようにした。stdin は readable、stdout/stderr は writable とした。
- `fork()` で親の fd table を子へコピーし、各 `File` の `refcnt` を `file::dup` で増やすようにした。`freeproc()` では fd table に残る open file を `file::close` して参照を落とす。
- `sys_read` / `sys_write` の fd 0/1/2 特別扱いを撤去し、`p.ofile[fd]` から `File` を引いて `file::read` / `file::write` に委譲する形にした。
- `write` syscall は全 byte の書き切りを保証せず、最大 128 byte を 1 回 `file::write` して実際に書けた byte 数を返す方針にした。全部書きたい user code は `write_all` を使う (D0036)。
- `read_line` は `write` の short write 許容に合わせ、出力に `write_all` を使うようにした。

### 詰まったこと / わかったこと

- `&[&[u8]]` は Rust の fat pointer 配列であり、kernel が期待する `char **` 互換の thin pointer 配列ではない。RV64 では各 `&[u8]` が `(ptr, len)` なので、kernel から見ると 2 個目の argv pointer として 1 個目の長さを読んでしまう。user → kernel の syscall ABI では `&[*const u8]` のような thin pointer 配列を渡す必要がある。
- `argv == NULL` は当面不正として扱うことにした。簡易 shell からの利用では少なくとも `argv[0]` を渡す方針にし、`copy_argv` は成功時 `argc >= 1` を契約にする。
- `KernelArgs` は `MAXARG = 16`, `MAXARGLEN = 128` でも約 2 KiB あり、1 page の kernel stack に置くと trap / syscall の既存フレームと合わせて stack overflow しやすい。実際、kernel stack 上に置いた経路では `exit` 後の page table cleanup で壊れた状態になった。
- `copy_argv` の `MAXARG` 判定は NULL 終端を読む前に行うと「ちょうど `MAXARG` 個 + NULL」も失敗してしまう。NULL pointer を読んでから、非 NULL の新しい引数を格納する直前に `argc >= MAXARG` を判定する必要がある。
- `push_argv` で user stack に事前配置する文字列・pointer array のレイアウトは、RISC-V psABI が直接規定する「関数呼び出し時のレジスタ引数」ではない。ただし `_start(argc, argv)` に `a0/a1` で渡す以上、stack 上の argv が `*const *const u8` として解釈できること、`sp` が 16-byte aligned であることは守る。
- `exec` は成功すると呼び出し元 program に戻らず、trap return 先の user context を別 program に置き換える syscall。普通の syscall と同じ `a0 = retval` 共通後処理に乗せると、次 program の初期引数を壊す。
- xv6 では `NFILE` は system 全体の opened file object 数、`NOFILE` は process ごとの fd table サイズ。fd は process-local な整数で、fd table entry が global `File` object を指す。
- `dup` は新しい file object を作らず、同じ open file description の `refcnt` を増やすだけ。fork 後の親子や将来の `dup` syscall は file offset などを共有する。
- `FileKind::Device` は当面 `major` だけを持ち、`minor` は xv6 の簡略化に寄せて file layer では扱わないことにした。console は character device の一種として `major = CONSOLE_MAJOR` で扱う。
- `FileKind` に kind 固有 field を置く設計なら、将来 inode file の offset は `FileKind::Inode { off, ... }` に置き、`match &mut f.kind` で更新する。xv6 のように `File` 本体に `off` / `major` を直置きするより、Rust では無効 field を避けやすい。
- `file::read` / `file::write` は user page table を知らず、kernel buffer に対してだけ動く。user memory の `copyin` / `copyout` は syscall layer の責務に残す。

### 検証

- `make build` が成功。
- 正常系 `argv = ["read_line", "test", NULL]`: `argc: 2`, `argv[0]: read_line`, `argv[1]: test` を確認し、その後 `read/write/exit/wait` まで成功。
- `argv == NULL`: `exec failed` で旧 child に戻り、child exit status `1`。
- 存在しない path `missing`: `exec failed` で旧 child に戻り、child exit status `1`。
- NUL 終端なし 128 byte 引数: `copyinstr` 失敗により `exec failed`。
- `MAXARG == 16` 個 + NULL: 成功し、`argc: 16`, `argv[0]..argv[15]` を確認。
- `MAXARG + 1` 個 + NULL: `exec failed`。
- FD/File 層導入後も `make build` が成功。
- `fork -> execv_cstr -> read_line -> read/write -> exit -> wait` を QEMU で確認。入力 `abcdef` に対して `read: abcdef` が出力され、child exit status `0` を parent が受け取った。

### 次にやること

- argv の一時テスト表示を常設テストにするか、通常の `read_line` からは外して shell 実装に進むかを決める。
- user library の `execv_cstr` は「各文字列が NUL 終端済み」「argv 配列が NULL 終端済み」という契約をコメントで明確にする。
- `KernelArgs::new()` が不要になっているので、残すなら用途を作る、不要なら削除する。
- `close` / `dup` syscall を user-visible にするか、まずは FS/RAM file 用の `open` に進むかを決める。
- `FileKind::Inode` / read-only RAM FS の導入に進む。`open(path)` が global file table slot を確保して fd に割り当てる経路が次の自然な拡張。
- 簡易 shell に進む場合は、1 行入力 → 空白分割 → `argv` pointer array 構築 → `fork` → child `execv_cstr` → parent `wait` の最小ループから始める。

### 参照

- xv6-riscv `kernel/exec.c` — `exec` 成功時の stack 上 argv 配置と `a0/a1` 設定。
- xv6-riscv `kernel/syscall.c` / `kernel/sysfile.c::sys_exec` — syscall return と `exec` の関係。
- xv6-riscv `kernel/file.c` / `kernel/file.h` — global file table、`filealloc` / `filedup` / `fileclose` / `fileread` / `filewrite`。
- xv6-riscv `kernel/proc.h::ofile` / `kernel/proc.c::fork` / `kernel/proc.c::freeproc` — per-process fd table と fork/close 時の file refcount 管理。
- POSIX `write(2)` — short write は成功として許容され、全 byte 書き切りが必要なら caller が loop する。
- RISC-V psABI — integer argument register (`a0` / `a1`) と stack pointer 16-byte alignment。

---

## 2026-05-08

### やったこと

- ファイルシステム方針を read-only RAM FS に決めた。永続化や書き込みは後回しにしつつ、将来 block device backed FS に寄せやすいよう `namei` / `readi` / inode を通す形にした (D0037)。
- `src/fs.rs` を追加。static inode tree として `/bin/read_line`, `/bin/read_file`, `/README.md` を持ち、絶対 path のみを扱う `namei(path)` と、offset 指定で file content を読む `readi(inode, off, dst)` を実装した。
- `InodeKind` は RAM FS の内部表現として private にし、外部には `InodeType::{File, Dir, Device}` だけを公開する形にした。これにより `sys_open` は必要最小限の分類だけを見て `FileKind` を作る。
- ELF loader に `load_elf_from_inode` を追加。`&[u8]` 前提を避け、ELF header / program header / segment を `fs::readi` で offset read する形にした (D0038)。
- `sys_exec` を embedded program table lookup から `fs::namei(path)` + `proc::exec_from_inode` に変更した。user 側は `/bin/read_line` / `/bin/read_file` の絶対 path を exec する。
- `FileKind::Inode { inode, off }` を追加し、`file::read` が `fs::readi` を呼んで open file description の offset を進めるようにした。
- `SYS_OPEN = 15` / `SYS_CLOSE = 21` と user 側 `open(path, flags)` / `close(fd)` wrapper を追加した。現時点では open flags は無視し、regular file は read-only、directory は失敗、device は device file として開く方針にした (D0039)。
- `user/src/bin/read_file.rs` を追加し、`open("/README.md")` → `read` loop → `close` を確認する検証用 program にした。
- `README.md` と `AGENTS.md` の到達点・設計メモを、scheduler / fd layer / read-only RAM FS / inode-based exec の現状に合わせて更新した。

### 詰まったこと / わかったこと

- RAM FS でも write support を入れると、可変長 data の確保・伸長・truncate・途中失敗 rollback などが急に重くなる。exec と shell 到達が目的なら、read-only に割り切る方がよい。
- RAM FS の file content は `&'static [u8]` として存在するため `fs::data()` のような API で直接 slice を渡すことは可能だが、disk-backed FS へ移ると成り立たない。loader は `fs::readi` による offset read に寄せた方が移行しやすい。
- `read_at` は open file の offset を更新せず、指定 offset から読む操作。今回は汎用 trait や `file::read_at` は導入せず、loader が inode に対して `fs::readi(inode, off, dst)` を使う形にした。
- `DirEnt.name` は `&'static str` より `&'static [u8]` の方が、`copyinstr` 後の path component とそのまま比較できる。Unix path name は本質的には byte sequence として扱うのが自然。
- `namei` は当面、絶対 path のみを扱う。`/` は root、末尾 slash / 連続 slash / 相対 path は失敗扱いにした。`"."` / `".."` / cwd は未対応。
- `readi` は EOF 以降では `0` を返す必要がある。`data.len() - off` を先に計算すると `off >= len` で underflow する。
- 同じ inode を 2 回 `open` すると別々の `File` object が作られ、offset は独立する。`dup` syscall を導入した場合だけ同じ `File` object を共有して offset も共有する設計になる。

### 検証

- `/bin/read_line` を `exec` し、`fs::namei` → `loader::load_elf_from_inode` → `fs::readi` 経由で従来通り `read_line` が動くことを確認。
- `/README.md` を `open` すると fd `3` が返り、`read` loop で README 全文を読めることを確認。
- `close(fd)` 後に `read(fd)` すると `-1` が返ることを確認。
- `/README.md` を 2 回 open すると fd `3` / fd `4` が割り当てられ、それぞれ先頭から読めることを確認。open file offset が独立している。

### 次にやること

- `dup` syscall を追加し、同じ `File` object を共有した fd 同士で offset が共有されることを確認する。
- `/dev/console` を device inode として RAM FS tree に追加し、`open("/dev/console")` の経路を確認する。
- `read_file` で確認した RAM FS の open/read/close を残すか、次の shell 検証用 program に置き換えるかを決める。
- 簡易 shell に進む。1 行入力 → 空白分割 → `argv` pointer array 構築 → `fork` → child `execv` → parent `wait` の最小ループから始める。

### 参照

- xv6-riscv `kernel/fs.c::namei` / `namex` / `readi`。
- xv6-riscv `kernel/exec.c` — inode から ELF header / program header / segment を読む構造。
- xv6-riscv `kernel/sysfile.c::sys_open` / `sys_close`。
- POSIX `open(2)` / `close(2)` / `read(2)` — fd allocation、close 後 fd invalidation、read の EOF `0`。

---

## 2026-05-09

### やったこと

- user library の `exec` / `open` wrapper で path を自動 NUL 終端するようにした。呼び出し側は `b"/bin/sh"` のような通常の byte slice を渡せばよく、`b"...\0"` を毎回書かなくてよい。
- heap がまだ無く、user stack も 1 page だけなので、user library 側の path buffer は kernel 側上限より小さい固定長配列にした。
- argv 付き shell は一旦見送り、最初の shell は「1 行を path として読み、そのまま `fork` → child `exec(path)` → parent `wait`」する path-only 仕様にした (D0040)。
- `user/src/bin/sh.rs` を追加。空行無視、前後空白の trim、長すぎる入力の読み捨て、存在しない path の失敗表示を入れた。
- `init` を `/bin/sh` 起動役に変更。shell が終了したら再起動し、shell 以外の子が終了した場合も `wait` で回収し続ける形にした。
- RAM FS の `/bin` から `read_line` を外し、`sh` と `read_file` を登録する形にした。
- `read_file` は `open(b"/README.md")` のように NUL 終端なしの path を使う形に更新した。

### 詰まったこと / わかったこと

- Rust の `&[&[u8]]` は shell 入力から組み立てるには固定長の slice 配列を別途持つ必要があり、heap なしの現段階では扱いが少し重い。
- kernel 側の `exec` は `argv` を扱えるが、user library / shell の公開 API は当面 path-only にしてもよい。`exec(path)` wrapper は `argv = [path, NULL]` を内部で作るため、kernel 側の `copy_argv` の `argc >= 1` 契約とも整合する。
- user library の自動 NUL 終端用 buffer は syscall 中に kernel が `copyinstr` / `copyin` で即時コピーするため、stack 上の一時配列で問題ない。
- `wait` は「子がいるがまだ zombie でない」場合だけ sleep し、子がいなければ `-1` を返す。init を reaper として常駐させるには、shell を起動し直す外側 loop と組み合わせるのが自然。
- PID 1 の `init` が失敗時に `exit` する設計は本来強くないが、現段階では「通常起きない致命的状態」として割り切る。

### 検証

- `make build` が成功。
- QEMU 上で `/bin/sh` の prompt まで到達することを確認。
- shell から `/bin/read_file` を実行し、`/README.md` を読めることを確認。
- 空行入力は無視されることを確認。
- 存在しない path (`/no/such`) は child 側で `[sh] exec failed` となり、shell prompt に戻ることを確認。

### 次にやること

- shell builtin として `exit` を入れるか検討する。
- argv を復活させる場合、固定長 argv parser と user library API の形を再検討する。
- `README.md` の RAM FS ステータスに残っている `/bin/read_line` 記述を、次の README 更新時に `/bin/sh` へ揃える。

### 参照

- xv6-riscv `user/init.c` — init が shell を起動し直す構造。
- xv6-riscv `user/sh.c` — `fork` / `exec` / `wait` による shell の基本形。
- xv6-riscv `kernel/proc.c::wait` / `exit` / `reparent`。

---

## 2026-05-11

### やったこと

- userland の動的メモリ確保に進む前提として、user address space layout を再整理した。
- user stack を ELF image 直後から高位アドレス `USER_STACK = MAXVA - 3 * PGSIZE` へ移動した (D0041)。
- `loader::load_elf` / `loader::load_elf_from_inode` の両方で、stack page を `USER_STACK` に map し、`sp = USER_STACK + PGSIZE` を返すようにした。
- `LoadedImage::sz` を stack 込みの address space size ではなく、低位 user image の page-aligned end / heap start として扱う形に整理した。
- page table teardown では `USER_STACK` が存在する場合だけ unmap/free するようにした。partial page table の cleanup で未 map stack を unmap して panic しないようにした。
- `fork` で `[0, sz)` だけでなく、高位 user stack page も child page table にコピーする `vm::uvmcopy_stack` を追加した。
- 新 layout に合わせて loader 周りの古いコメントを更新した。
- 変更を `2ca12fb Move user stack to high address` として commit / push した。
- `sbrk` syscall を追加した。syscall 番号は xv6 に合わせて `12` とし、現時点では正の increment のみ対応する。
- `vm::uvmalloc` を追加し、`sbrk` で増えた break の page-rounded 差分だけ user page を `PTE_U | PTE_R | PTE_W` で map するようにした。
- userland に `GlobalAlloc` 実装 `UserAllocator` を追加した。16-byte align まで対応し、first-fit free list / split / address-ordered insert / coalesce を行う (D0042)。
- `Box` / `Vec` を使う検証用 user program `/bin/alloc_test` を追加し、RAM FS に登録した。
- allocator 導入により user ELF に `.bss` が出るようになったため、現 loader の page-aligned PT_LOAD 前提に合わせて `user/linker.ld` の `.bss` を 4096 byte align にした。
- user library の `exec` / `open` wrapper を `alloc::ffi::CString` ベースに変更した。呼び出し側は NUL 終端なしの byte slice を渡し、wrapper が heap 上で NUL 終端済み buffer を作る。
- `exec(path, argv)` の user-facing API を `path: &[u8]`, `argv: &[&[u8]]` にし、内部で thin pointer 配列 + NULL 終端へ変換するようにした。
- shell が入力行を ASCII 空白で分割し、`argv` を child `exec` に渡すようにした。
- command name に `/` が含まれない場合は shell が `/bin/<cmd>` を exec path として使う簡易 command lookup を入れた。`argv[0]` は入力された command name のまま渡す (D0043)。
- `/bin/read_file` を Unix-like な名前の `/bin/cat` に変更し、RAM FS の登録名と埋め込み ELF も `cat` に揃えた。現段階では `cat FILE` の 1 ファイル読み取りのみ対応し、引数なし stdin echo は EOF 未対応のため入れない。
- 次の FS 方針を整理した。現在の static read-only RAM FS に `ls` 用の暫定 directory read を足すのではなく、RAM-backed block array 上に xv6 風の inode FS を作る。buffer cache と crash recovery log は省くが、`Dinode` / inode cache / block bitmap / direct + single indirect / directory-as-file は持つ (D0044)。
- 新 FS のロック方針も整理した。coarse な FS 全体 lock ではなく、`ICACHE_LOCK`、`ITABLE_LOCK`、`BALLOC_LOCK`、各 `inode.lock` に分ける。`readi` / `writei` は caller が inode lock を持つ契約にする。

### 詰まったこと / わかったこと

- `sz` を heap end / image end の意味にすると、`uvmcopy(parent.sz)` だけでは高位 stack がコピーされない。固定 user mapping として別途コピーする必要がある。
- `fork` 後も親子の user VA layout は同じなので、親の trapframe をコピーした後に user `sp` を書き換える必要はない。`a0` だけ child return value の `0` にすればよい。
- `uvmunmap` は「対象が map 済み」という強い契約を保ち、optional な `USER_STACK` の存在確認は `proc_freepagetable` 側で行う方が影響範囲が小さい。
- user stack を高位に分離すると、ELF image の直後を heap start として扱いやすくなる。次に `sbrk` / userland allocator へ進みやすい。
- `sbrk` の戻り値は新 break ではなく旧 break。user allocator はこれを新しい heap chunk の先頭として使う。
- `p.sz` は byte 単位の current break として保持し、実際の page allocation は `round_up(oldsz)..round_up(newsz)` の差分で行う。
- user allocator が page 単位で `sbrk` すれば syscall 回数を抑えられ、かつ `sbrk` が返す chunk base は 16-byte alignment を自然に満たす。
- `alloc_test` の最初の失敗原因は allocator そのものではなく、`.bss` 用の `PT_LOAD` が page 途中の VA から始まり、loader の `p_vaddr` page-aligned assert に引っかかったことだった。
- `alloc::ffi::CString` は `no_std + alloc` でも使えるため、path / argv の NUL 終端 helper を自作する必要はなかった。
- shell 入力は `read` 由来の byte列なので、`exec` wrapper の公開 API を `&str` にすると UTF-8 validation が余計な責務になる。Unix path / argv は NUL を含まない byte string として扱う方が自然。
- shell の command lookup は「`/` を含むか」で分けると、将来 relative path を導入しても `./foo` や `dir/foo` を path として扱える。slash なし command だけを `/bin` から探す形は、将来の `PATH = ["/bin"]` に一般化しやすい。
- `Dinode` は RAM block array 上の disk-format inode、`Inode` は kernel memory 上の cache object として分ける。RAM-backed でも二重化にはなるが、将来 block device / buffer cache に進むときに上位構造を保ちやすい。
- inode cache は同じ `inum` に同じ memory `Inode` を返すことで coherence を保つ。`iget` は slot/refcount だけを扱い、`ilock` で必要に応じて `Dinode` を lazy load する。
- file data の通常 read/write は inode lock で直列化できるが、block bitmap、inode table block、inode cache slot/refcount は別の共有構造なので、それぞれ別 lock が必要になる。

### 検証

- `make build` が成功。
- QEMU 上で `/bin/alloc_test` を実行し、`alloc_test ok` と exit code `0` を確認した。
- `make build` が成功。`/bin/cat` への rename、shell argv 分割、`CString` ベースの `exec` / `open` wrapper がビルドを通ることを確認した。

### 次にやること

- allocator の制限 (`align > 16` 未対応、invalid free / double free 未検出、single-thread 前提) を必要に応じてコメントや README に残す。
- shell から `cat /README.md` のように argv 付き exec できることを QEMU 上で確認する。
- `cat` を `cat FILE...` に拡張するか、当面 `cat FILE` のみとするかを決める。
- RAM-backed inode FS の最初のマイルストーンとして、block layout と `Dinode` / inode cache / bitmap allocator の設計を具体化する。
- 近いうちに null guard page (`USER_BASE = PGSIZE`) を導入するか再検討する。
- loader を本来の ELF semantics に近づけ、page 途中から始まる `PT_LOAD` や同一 page を共有する segment を扱えるようにするか検討する。

### 参照

- xv6-riscv `kernel/memlayout.h` — `TRAMPOLINE` / `TRAPFRAME` / high user stack 周辺の配置。
- xv6-riscv `kernel/vm.c::uvmcopy` / `kernel/proc.c::fork` — user address space copy の考え方。
- xv6-riscv `kernel/sysproc.c::sys_sbrk` / `kernel/proc.c::growproc` / `kernel/vm.c::uvmalloc`。
- RISC-V Privileged Spec — Sv39 virtual address layout と canonical address 制約。
