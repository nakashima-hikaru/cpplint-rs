# 実装済みルール一覧

`cpplint-rs` で現在実装されているルールのリストです。各カテゴリごとに個別に列挙しています。

自動Fix列の意味:

- `○`: 自動Fix対応
- `△`: 一部の違反パターンのみ自動Fix対応
- `-`: 自動Fixなし

| ルールカテゴリ | ファミリー | フェーズ | 自動Fix | 概要 |
| :--- | :--- | :--- | :---: | :--- |
| `legal/copyright` | copyright | RawSource | - | ファイル冒頭の著作権ボイラープレートの確認 |
| `build/header_guard` | headers | FileStructure | ○ | ヘッダーガード（#ifndef, #define, #endif）の正当性確認 |
| `build/include` | headers | FileStructure | △ | インクルードの整合性と重複の確認 |
| `build/include_alpha` | headers | FileStructure | △ | インクルードセクション内でのアルファベット順の確認 |
| `build/include_order` | headers | FileStructure | △ | インクルードの優先順位（C++, C, OS等）の確認 |
| `build/include_subdir` | headers | FileStructure | - | インクルードパスのサブディレクトリ指定の確認 |
| `build/include_what_you_use` | headers | FileStructure | △ | 不要なインクルードや不足している前方宣言の確認 |
| `whitespace/blank_line` | whitespace | Line | ○ | 余分な、または不足している空行の確認 |
| `whitespace/braces` | whitespace | Line | △ | 中括弧（{, }）の前後のスペースと配置の確認 |
| `whitespace/comma` | whitespace | Line | ○ | カンマ（,）の後のスペースの確認 |
| `whitespace/comments` | whitespace | Line | ○ | コメント（//, /* */）の前後と内部のスペースの確認 |
| `whitespace/empty_conditional_body` | whitespace | Line | ○ | if/else等の空のブロック体に関する確認 |
| `whitespace/empty_if_body` | whitespace | Line | ○ | 空のif文ブロックに関する確認 |
| `whitespace/empty_loop_body` | whitespace | Line | ○ | 空のループ（for, while）ブロックに関する確認 |
| `whitespace/end_of_line` | whitespace | Line | ○ | 行末の余分な空白の確認 |
| `whitespace/ending_newline` | whitespace | Finalize | ○ | ファイル末尾が改行で終わっているかの確認 |
| `whitespace/forcolon` | whitespace | Line | ○ | 範囲ベースfor文のコロン前後のスペースの確認 |
| `whitespace/indent` | whitespace | Line | △ | インデント（スペース2文字等）の正当性確認 |
| `whitespace/indent_namespace` | whitespace | Line | ○ | 名前空間内でのインデントルールの確認 |
| `whitespace/line_length` | whitespace | Line | - | 1行の長さ（標準80文字）の制限確認 |
| `whitespace/newline` | whitespace | Line | △ | 改行コードや改行位置の確認 |
| `whitespace/operators` | whitespace | Line | △ | 演算子前後のスペースの確認 |
| `whitespace/parens` | whitespace | Line | △ | 括弧（( )）内部のスペースと関数名との間のスペース確認 |
| `whitespace/semicolon` | whitespace | Line | ○ | セミコロン（;）の前の不要なスペースの確認 |
| `whitespace/tab` | whitespace | Line | ○ | タブ文字の使用禁止（スペース推奨）の確認 |
| `whitespace/todo` | whitespace | Line | △ | TODOコメントのフォーマット（TODO(username):）の確認 |
| `runtime/arrays` | runtime | Line | - | 固定長配列よりもコンテナ（std::array等）を推奨する確認 |
| `runtime/casting` | runtime | Line | - | Cスタイルのキャストの使用禁止とC++キャストの推奨確認 |
| `runtime/explicit` | runtime | Line | - | 1引数コンストラクタへのexplicit付与の確認 |
| `runtime/init` | runtime | Line | - | 変数の初期化に関する確認 |
| `runtime/int` | runtime | Line | - | 型サイズが不明確なint等の使用に関する確認 |
| `runtime/invalid_increment` | runtime | Line | - | イテレータ等での後置インクリメントの非推奨確認 |
| `runtime/member_string_references` | runtime | Line | - | メンバ変数へのstd::string参照保持の危険性確認 |
| `runtime/memset` | runtime | Line | ○ | memsetの使用に関する安全性の確認 |
| `runtime/operator` | runtime | Line | - | 演算子オーバーロードの適切な使用確認 |
| `runtime/printf` | runtime | Line | - | printf系列の関数の使用とフォーマットの確認 |
| `runtime/printf_format` | runtime | Line | △ | printf系のフォーマット文字列と引数の整合性確認 |
| `runtime/references` | runtime | Line | - | 非const参照引数の使用（ポインタ推奨）の確認 |
| `runtime/string` | runtime | Line | - | std::stringの使用に関する効率と安全性の確認 |
| `runtime/threadsafe_fn` | runtime | Line | - | スレッドセーフでない関数（strtok等）の使用確認 |
| `runtime/vlog` | runtime | Line | ○ | VLOGマクロの使用に関する確認 |
| `build/c++11` | readability | Line | - | C++11関連の構文と機能の使用確認 |
| `build/c++17` | readability | Line | - | C++17関連の構文と機能の使用確認 |
| `build/deprecated` | readability | Line | - | 非推奨（deprecated）な機能の使用確認 |
| `build/endif_comment` | readability | Line | ○ | #endifの後の対応するマクロ名コメントの確認 |
| `build/explicit_make_pair` | readability | Line | ○ | std::make_pairの明示的な型指定の抑制確認 |
| `build/forward_decl` | readability | Line | ○ | 前方宣言の適切な使用とインクルードの置換確認 |
| `build/namespaces_headers` | readability | Line | - | ヘッダーファイルでのusing namespace禁止の確認 |
| `build/namespaces_literals` | readability | Line | - | リテラル名前空間の使用に関する確認 |
| `build/namespaces` | readability | Line | - | 名前空間の適切な宣言と終了の確認 |
| `build/storage_class` | readability | Line | ○ | staticやextern等のストレージクラスの配置確認 |
| `readability/alt_tokens` | readability | Line | ○ | and/or/not等の代替トークンの使用確認 |
| `readability/braces` | readability | Line | △ | ブロックの開き・閉じ括弧のスタイル確認 |
| `readability/casting` | readability | Line | - | キャストの可読性に関する確認 |
| `readability/check` | readability | Line | ○ | CHECKマクロの使用に関する確認 |
| `readability/constructors` | readability | Line | - | コンストラクタの宣言と初期化子リストの可読性確認 |
| `readability/fn_size` | readability | Line | - | 関数の行数過大（複雑度）の確認 |
| `readability/inheritance` | readability | Line | △ | 継承に関するスタイル（virtual/override）の確認 |
| `readability/multiline_comment` | readability | Line | - | 複数行コメントのフォーマット確認 |
| `readability/multiline_string` | readability | Line | - | 複数行にわたる文字列リテラルの配置確認 |
| `readability/namespace` | readability | Line | ○ | 名前空間の閉じ位置のコメント確認 |
| `readability/nolint` | readability | Line | - | // NOLINT の適切な使用確認 |
| `readability/nul` | readability | Line | - | NUL文字バイトの混入確認 |
| `readability/strings` | readability | Line | - | 文字列リテラルの配置と連結の確認 |
| `readability/todo` | readability | Line | - | TODOコメントの可読性確認 |
| `readability/utf8` | readability | Line | - | ソースコードのUTF-8エンコーディング確認 |
