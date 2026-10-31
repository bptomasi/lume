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
