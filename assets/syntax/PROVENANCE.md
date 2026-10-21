# Syntax definitions

Unmodified text definitions from SublimeHQ/Packages revision
`fa6b8629c95041bf262d4c1dab95c456a0530122`, the package revision qualified by
Syntect 5.3.0's test corpus. Their accompanying permissive license is retained
in `LICENSE`. The grant allows copying, use, modification, sale and distribution.
The Rust directory also supplies MIT terms, retained in `Rust-license.txt`.

Lume loads only these embedded definitions. It never loads grammar files or
regex patterns from a feed. Its own colors are defined by the reader owner.

`LumeSQL.sublime-syntax` is a project-authored MIT overlay. It includes the
unmodified SQL grammar and adds command keywords such as CALL, ATTACH and
CONNECT. Syntect owns matching, string/comment contexts and multiline state;
the overlay is compiled and linked with the same reviewed grammar pack.

## Expanded language set (2026-10-17)

The pack contains 21 definitions plus plain text. C++, CSS, HTML, Java,
JavaDoc, JavaScript and YAML use the same SublimeHQ revision above. Embedded
JavaScript/Python regular expressions are included as their owning grammars
require them. The build rejects unresolved cross-grammar contexts.
These additional unmodified SublimeHQ files have the following Git blob IDs:

| Upstream path | Git blob SHA-1 |
| --- | --- |
| `C++/C++.sublime-syntax` | `398b8cfb30e64fa489b012eaff90f8c788f61e7b` |
| `CSS/CSS.sublime-syntax` | `9f9862dad45520071486a6b507239cdd9a6f8c4d` |
| `HTML/HTML.sublime-syntax` | `861c6cc67cd2e987fd8dcd695717b9c90fdf06ab` |
| `Java/Java.sublime-syntax` | `f50632d689a3da04f4d4781121b52be577925178` |
| `Java/JavaDoc.sublime-syntax` | `cab24c6148ae565ed99677c64aeb92a319d1692a` |
| `JavaScript/JavaScript.sublime-syntax` | `e72449ac794b4bfa3e0fa2ec65037a3c4690f4cf` |
| `JavaScript/Regular Expressions (JavaScript).sublime-syntax` | `66e835761656470d2a0e7c7467804b3af2fa748a` |
| `YAML/YAML.sublime-syntax` | `9f0197ac6a55693e9236b4eabf125484530b0ac4` |
| `Python/Regular Expressions (Python).sublime-syntax` | `88346617753030749189a8f8df22f837a32e6683` |

Terraform/HCL uses the compatible text definition in
[SublimeText/Terraform](https://github.com/SublimeText/Terraform/tree/54d8350c3c5929c921ea7561c932aa15e7d96c48)
revision `54d8350c3c5929c921ea7561c932aa15e7d96c48`, also pinned by bat's
syntax corpus. Its MIT notice is retained in `Terraform-LICENSE.txt`.
This selects a qualified grammar version rather than newer Sublime syntax
inheritance features that our pinned Syntect cannot load.

TypeScript uses the converted, self-contained Sublime syntax from
[bat](https://github.com/sharkdp/bat/tree/979ba22628bc9d8171f2cffca2bd5c90c9fc0a9e)
revision `979ba22628bc9d8171f2cffca2bd5c90c9fc0a9e`. Its original grammar is
[Microsoft/TypeScript-Sublime-Plugin](https://github.com/microsoft/TypeScript-Sublime-Plugin/tree/ba45efd058df5111837e30fb9598cfc8cbd51095)
revision `ba45efd058df5111837e30fb9598cfc8cbd51095`. Retain Microsoft's
Apache-2.0 terms in `TypeScript-LICENSE.txt` and bat's MIT notice in
`bat-LICENSE.txt`. JSX/TSX definitions are not included.

| Local file | Git blob SHA-1 |
| --- | --- |
| `Terraform.sublime-syntax` | `d54c71aaf4d4d10b60cb6348aa011b345f8d0881` |
| `Terraform-LICENSE.txt` | `e8d0f3a1db3c2c29ee5a32f0044e80056fd495d9` |
| `TypeScript.sublime-syntax` | `68c9b4a942f2e11b2b50b8240a4e9d44d2431bfb` |
| `TypeScript-LICENSE.txt` | `e06d2081865a766a8668acc12878f98b27fc9ea0` |
| `bat-LICENSE.txt` | `2374e6c3f82b87e31f06b39e1c2c05beda16242a` |

All added grammar and notice bytes were checked against these upstream Git
blob identities, including the original absence of a trailing newline in the
JavaScript and Terraform definitions. Long upstream patterns are kept intact.
Only our generated pack is deserialized at runtime; no upstream binary pack
or dynamically supplied regular expression is accepted.
