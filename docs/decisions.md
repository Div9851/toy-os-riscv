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
- 状態: 採用
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
