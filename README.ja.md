# own-repos-curator-to-hatena

# これは何？

- own-repos-curator から呼び出すアプリです。
- repos.jsonを元に、はてなブログへの投稿を行うためのmarkdownファイルを生成します。
- 生成したmarkdownファイルは決め打ちでcommit pushされます。
- 自分用なので他人が使える作りになっていません。
- 頻繁に破壊的変更を行います。

# 用途

- 自分のリポジトリ群の説明文を、はてなブログに投稿する用

# インストール

Rustが必要です。

```
cargo install --force --git https://github.com/cat2151/own-repos-curator-to-hatena
```

# 実行

通常実行:

```
own-repos-curator-to-hatena
```

ローカル出力のみ:

```
own-repos-curator-to-hatena --dry-run
```

自己更新:

```
own-repos-curator-to-hatena update
```

更新確認:

```
own-repos-curator-to-hatena check
```

# 注意

`update` には Python が必要です。
