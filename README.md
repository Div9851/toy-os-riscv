# toy-os-riscv

RISC-V (rv64gc) 向けの Unix-like な OS を Rust で実装する学習プロジェクト。

## ターゲット

- アーキテクチャ: `riscv64gc-unknown-none-elf` (rv64gc)
- マシン: QEMU `virt` ボード
- ファームウェア: OpenSBI (S-mode から起動)
- 当面はシングルコア前提

## ゴール

シェルが動くところまでを短期目標とし、最低限以下を実装する:

- プロセス管理 (コンテキストスイッチ / スケジューラ / システムコール)
- メモリ管理 (Sv39 ページング / 物理ページアロケータ / カーネルヒープ)
- 簡易ファイルシステム
- ユーザランドの簡易シェル

その後、SMP 対応・ネットワークプロトコルスタック・システムコールの拡充に進む予定。

## ステータス

現在は以下まで到達している:

- OpenSBI から S-mode kernel を起動
- SBI console / UART による kernel 側の出力
- kernel layout / trampoline / trapframe の基本構成
- timer interrupt と external interrupt の入口
- 物理ページアロケータ
- Sv39 ページテーブルによる kernel / user address space の切り替え
- embedded user ELF (`user/src/bin/init.rs`) のロード
- U-mode への遷移
- `ecall` ベースの syscall dispatch
- `write(1|2, buf, len)` による user program からの文字列出力
- `exit(code)` の暫定実装

未実装または今後の主な作業:

- 複数プロセス、コンテキストスイッチ、スケジューラ
- process exit の本実装
- user exception を kernel panic ではなく process kill にする処理
- kernel heap / `alloc`
- file system
- shell

## ビルド / 実行

ビルドと実行は `make` で行う。

```sh
make build
make run
```

`make build` は先に user program を release build し、その ELF を kernel に `include_bytes!` で埋め込む。

QEMU は `virt` machine、OpenSBI、single hart、128 MiB RAM で起動する。
