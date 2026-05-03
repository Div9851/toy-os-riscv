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
