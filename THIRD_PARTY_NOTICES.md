# Third-party notices

Mutie's original code is licensed under MIT OR Apache-2.0. Material retained from
other projects remains under its upstream license; the dual-license choice does
not relicense that material.

## What is included

The benchmark projects themselves are not vendored here. They are checked out
separately when running the audit scripts. This repository retains our extra
tests and audit tools, result records, and small upstream excerpts in patches,
mutation diffs, and diagnostic output. The Vitest adapter also contains exact
source anchors and modified snippets used to instrument its isolated runner.

The notices below accompany those excerpts. They cover benchmark material named
for the corresponding project under `benchmarks/`, including nested fixtures and
results. The Vitest notice also applies to the upstream snippets in
`src/vitest/lifecycle.rs`. Original test additions and audit tools written for
Mutie remain available under Mutie's dual license.

## Upstream licenses

Links identify the revisions audited for the retained material. Notice files
preserve the upstream text, including existing copyright years and attribution.
For dual-licensed UCD Generate and Marathon excerpts, the MIT alternative is used.

| Source | License for retained excerpts | Included text |
| --- | --- | --- |
| [a-poor/marathon](https://github.com/a-poor/marathon/blob/750e21cf7778f0b59c19df194cdb85d15afe2963/LICENSE-MIT.txt) | MIT | [Notice](licenses/third-party/marathon-mit.txt) |
| [artmsilva/bragkit](https://github.com/artmsilva/bragkit/blob/458d31127f1d3a6f3fd28afd2d7a4741ebc648a7/LICENSE) | MIT | [Notice](licenses/third-party/bragkit.txt) |
| [bene-art/betting-math-kit](https://github.com/bene-art/betting-math-kit/blob/0bd3c25e93df8974f432310835768d77a5e45d3e/LICENSE) | MIT | [Notice](licenses/third-party/betting-math.txt) |
| [BurntSushi/ucd-generate](https://github.com/BurntSushi/ucd-generate/blob/e8fa937f0cac643669dcaf5edac2785b15cab917/LICENSE-MIT) | MIT | [Notice](licenses/third-party/ucd-mit.txt) |
| [burpeesDaily/sphinx-sample-code](https://github.com/burpeesDaily/sphinx-sample-code/blob/68d688fb4fe9c867c77a9d54f4fdb46a58dea34e/LICENSE) | MIT | [Notice](licenses/third-party/trees.txt) |
| [carlos-sweb/pp-is](https://github.com/carlos-sweb/pp-is/blob/1d5b3bac804e1e67da289f3d2a14bd0705a383fb/LICENSE.txt) | MIT | [Notice](licenses/third-party/pp-is.txt) |
| [chrisdoherty4/python-case-converter](https://github.com/chrisdoherty4/python-case-converter/blob/4b625748db63f7f20353b5b0815e42122d739314/LICENSE) | MIT | [Notice](licenses/third-party/case-converter.txt) |
| [google/go-cmp](https://github.com/google/go-cmp/blob/v0.7.0/LICENSE) | BSD-3-Clause | [Notice](licenses/third-party/go-cmp.txt) |
| [gramiojs/composer](https://github.com/gramiojs/composer/blob/3de15b846e3f5ef46cf8aebd5d136908baa43b54/LICENSE) | MIT | [Notice](licenses/third-party/composer.txt) |
| [hukkin/tomli-w](https://github.com/hukkin/tomli-w/blob/1210bb6f5708a4dc084cc3deb4a3a3e3939f4d6d/LICENSE) | MIT | [Notice](licenses/third-party/tomli-w.txt) |
| [hyparam/hyparquet](https://github.com/hyparam/hyparquet/blob/49c88a44fa04c10905821e9a6a7b7a08bf091557/LICENSE) | MIT | [Notice](licenses/third-party/hyparquet.txt) |
| [joho/godotenv](https://github.com/joho/godotenv/blob/v1.5.1/LICENCE) | MIT | [Notice](licenses/third-party/godotenv.txt) |
| [jpvanhal/inflection](https://github.com/jpvanhal/inflection/blob/88eefaacf7d0caaa701af7c8ab2d0ab3f17086f1/LICENSE) | MIT | [Notice](licenses/third-party/inflection.txt) |
| [junegunn/fzf](https://github.com/junegunn/fzf/blob/416aff86e9d550d0a34af9d3cc12dfede25354c3/LICENSE) | MIT | [Notice](licenses/third-party/fzf.txt) |
| [JustinJLeopard/route-mini](https://github.com/JustinJLeopard/route-mini/blob/35a3db157731ba0073d66f432b69b997ce2e9f01/LICENSE) | MIT | [Notice](licenses/third-party/route-mini.txt) |
| [Kozea/tinycss2](https://github.com/Kozea/tinycss2/blob/853bc5a5661a59c7a9795a4e2758d0081808ce06/LICENSE) | BSD-3-Clause | [Notice](licenses/third-party/tinycss2-121.txt) |
| [MartiAble/scrubify-py](https://github.com/MartiAble/scrubify-py/blob/558265805d942c03d2e6d8522baa0ca2992ec067/LICENSE) | MIT | [Notice](licenses/third-party/scrubify.txt) |
| [matplotlib/cycler](https://github.com/matplotlib/cycler/blob/d1737eb0a42bd451e8770cef245ebe60ebb36bfc/LICENSE) | BSD-3-Clause | [Notice](licenses/third-party/cycler.txt) |
| [subresource-integrity/sri-dynamic-loader](https://github.com/subresource-integrity/sri-dynamic-loader/blob/370652c304d6fee0c59cfb51bf097e72f6ebf13a/LICENSE) | MIT | [Notice](licenses/third-party/sri.txt) |
| [unjs/pathe](https://github.com/unjs/pathe/blob/bc7477a01f0bd60ada017add8142c9f9d69ccdc5/LICENSE) | MIT | [Notice](licenses/third-party/pathe.txt) |
| [vitest-dev/vitest](https://github.com/vitest-dev/vitest/blob/v4.1.9/LICENSE) | MIT | [Notice](licenses/third-party/vitest.txt) |
| [weixu365/pluralizer-py](https://github.com/weixu365/pluralizer-py/blob/860a8e4514045896e7359aa519888e2f7c93f35c/LICENSE) | MIT | [Notice](licenses/third-party/pluralizer.txt) |
| [yukiharukonishi/openclaw-cc](https://github.com/yukiharukonishi/openclaw-cc/blob/e323faa446f03b5438fa9edc58781b4ef8dd52df/package.json) | MIT declaration; see below | [Terms](licenses/third-party/openclaw-mit.txt) |

OpenClaw's pinned `package.json` declares MIT, and its README also states MIT,
but that tree contains no standalone license or copyright notice. The linked
file supplies the standard MIT terms; no copyright holder or year has been
invented. Source attribution remains to the repository and revision above.

## Dependencies and new benchmark material

Cargo dependencies are resolved through `Cargo.lock`; their source trees are not
vendored in this repository. The projects' own licenses continue to apply. A
future binary distribution should include the notices required by its bundled
dependencies as well as these source notices.

When retaining new third-party code or excerpts, record the project and exact
revision here, include its applicable license text, and identify changes. Keep
full upstream checkouts and installed dependency trees outside this repository.
