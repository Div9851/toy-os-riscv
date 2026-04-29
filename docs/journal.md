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

### 詰まったこと / わかったこと

- xv6-riscv は OpenSBI に乗っていない。`entry.S` + `start.c` で M-mode 初期化 (PMP・`medeleg`/`mideleg`・タイマトランポリン) をしてから `mret` で S-mode に降りている。
- OpenSBI に乗ると Console 以外にも以下を抽象化してくれる:
  - タイマ (`sbi_set_timer`) — `mtimecmp` は本来 M-mode 専用なので、自力でやると M-mode トランポリンが必要。
  - HSM (`sbi_hart_start/stop/suspend`) — SMP の bring-up を任せられる。
  - IPI (`sbi_send_ipi`)、System Reset (`sbi_system_reset`)。
- Sv39: 39 bit VA、canonical 制約で上下 256 GiB ずつの 2 領域 (合計 512 GiB)。3 段ページテーブル、ページサイズは 4 KiB / 2 MiB / 1 GiB。物理は理論上 56 bit (PPN 44 bit + offset 12 bit)。
- OpenSBI から飛び込んできた直後の状態: S-mode、`satp = 0`、`a0 = hartid`、`a1 = DTB` 物理アドレス、エントリ `0x8020_0000`。

### 次にやること

「Hello, world を SBI Console に出す」までを 5 段階で進める:

1. 空の `no_std` カーネルでビルドが通る (`Cargo.toml` / `rust-toolchain.toml` / `.cargo/config.toml` / `linker.ld` / `src/main.rs`)。
2. QEMU で起動して、OpenSBI のバナー後に kernel に jump して止まる。
3. SBI で `H` 1 文字を出す。
4. `"Hello, world\n"` を出す。
5. `core::fmt::Write` 実装と `print!`/`println!` マクロまで整備する。

### 参照

- RISC-V Privileged Spec — Supervisor-Level ISA (Sv39 / `satp`)。
- RISC-V SBI Spec — Legacy Extension の Console Putchar (EID = `0x01`)。
- xv6-riscv の `kernel/entry.S`、`kernel/start.c` (今回は採用しないが参考)。
