# Image qualification fixtures

`request-path.svg` is the existing Lume architectural diagram fixture.

`astronaut.png` is the unmodified scikit-image 0.25.2 photograph of NASA
astronaut Eileen Collins, 512 × 512 RGB. Scikit-image identifies the NASA
image as public domain, with no known copyright restrictions:
https://scikit-image.org/docs/stable/api/skimage.data.html#skimage.data.astronaut

Retrieved from:
https://raw.githubusercontent.com/scikit-image/scikit-image/v0.25.2/skimage/data/astronaut.png

SHA-256:
`88431cd9653ccd539741b555fb0a46b61558b301d4110412b5bc28b5e3ea6cb5`.

Additional benchmark fixtures are generated deterministically by
`examples/sixel_profile/fixtures.rs`. No downloaded data or image fixture
is added to the application's runtime image cache.

Three additional unmodified photographs come from scikit-image v0.25.2.
The [official image documentation](https://scikit-image.org/docs/stable/api/skimage.data.html)
identifies them as CC0:

| File | Author | SHA-256 |
| --- | --- | --- |
| `camera.png` | Lav Varshney | `b0793d2adda0fa6ae899c03989482bff9a42d3d5690fc7e3648f2795d730c23a` |
| `chelsea.png` | Stefan van der Walt | `596aa1e7cb875eb79f437e310381d26b338a81c2da23439704a73c4651e8c4bb` |
| `coffee.png` | Rachel Michetti | `cc02f8ca188b167c775a7101b5d767d1e71792cf762c33d6fa15a4599b5a8de7` |

Original files, with hashes checked before import:

- [camera.png](https://raw.githubusercontent.com/scikit-image/scikit-image/v0.25.2/skimage/data/camera.png)
- [chelsea.png](https://raw.githubusercontent.com/scikit-image/scikit-image/v0.25.2/skimage/data/chelsea.png)
- [coffee.png](https://raw.githubusercontent.com/scikit-image/scikit-image/v0.25.2/skimage/data/coffee.png)

The `ui` and `dense` scenarios are repository-authored deterministic SVGs,
not additional photographs. `rgba16` is a real 16-bit RGBA PNG with hidden
RGB, partial alpha, and values not restricted to eight-bit multiples. The
benchmark exports source bytes once and supplies the same files to both
executables; resizing, mattes and zoom are explicitly recorded scenarios.
