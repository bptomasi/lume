# Article image dependency notices

These image/SVG additions use permissive licenses. Cargo.lock pins registry
checksums. Exact license texts are retained in THIRD_PARTY_LICENSES.txt, grouping
identical texts while listing every package/file owner. Platform-only packages
are included conservatively; this is an inventory, not an advisory audit.

icy_sixel 0.1.3 omits a Cargo license expression; its published LICENSE contains
the MIT grant and Hayaki Saito copyright. base64-simd/vsimd 0.8.0 omit license
files from their published archives. Their Cargo metadata declares MIT; the
retained upstream LICENSE is from their shared .cargo_vcs_info.json revision
[d74c030d9dc4f3cae02146d1f497ff62726ef09a](https://github.com/Nugine/simd/blob/d74c030d9dc4f3cae02146d1f497ff62726ef09a/LICENSE).

| Package | Version | Declared license / reviewed grant | Registry SHA-256 |
| --- | --- | --- | --- |
| arrayref | 0.3.9 | BSD-2-Clause | 76a2e8124351fda1ef8aaaa3bbd7ebbcb486bbcd4225aca0aa0d84bb2db8fecb |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 | d3fb67a6e08acf24fdeccbac2cb6ac4305825bd1f117462e0e6f2f193345ad56 |
| base64-simd | 0.8.0 | MIT | 339abbe78e73178762e23bea9dfd08e697eb3f3301cd4be981c0f78ba5859195 |
| block-buffer | 0.12.1 | MIT OR Apache-2.0 | d2f6c7dbe95a6ed67ad9f18e57daf93a2f034c524b99fd2b76d18fdfeb6660aa |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT | 95832e849adfb21180ccb6826a99da14e5d266ae5c2e668e1602cf234f153797 |
| bytemuck_derive | 1.12.1 | Zlib OR Apache-2.0 OR MIT | 6a1f896587b6f2c069c73d2f0913e2d590c3990285cd2f0b6aa02b786b4c679c |
| byteorder-lite | 0.1.0 | Unlicense OR MIT | 8f1fe948ff07f4bd06c30984e69f5b4899c516a3ef74f34df92a2df2ab535495 |
| color_quant | 1.1.0 | MIT | 3d7b894f5411737b7867f4827955924d7c254fc9f4d91a6aad6b097804b1018b |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 | 5ca28b0ae3115b884660db4118d803791fd6756b6e88f39c0f3f7859060d7566 |
| crypto-common | 0.2.2 | MIT OR Apache-2.0 | ce6e4c961d6cd6c9a86db418387425e8bdeaf05b3c8bc1411e6dca4c252f1453 |
| data-url | 0.3.2 | MIT OR Apache-2.0 | be1e0bca6c3637f992fc1cc7cbc52a78c1ef6db076dbf1059c4323d6a2048376 |
| digest | 0.11.3 | MIT OR Apache-2.0 | f1dd6dbb5841937940781866fa1281a1ff7bd3bf827091440879f9994983d5c2 |
| euclid | 0.22.14 | MIT OR Apache-2.0 | f1a05365e3b1c6d1650318537c7460c6923f1abdd272ad6842baa2b509957a06 |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 | 1e6853b52649d4ac5c0bd02320cddc5ba956bdb407c4b75a2c6b75bf51500f8c |
| float-cmp | 0.9.0 | MIT | 98de4bbd547a563b716d8dfa9aad1cb19bfab00f4fa09a6a4ed21dbcf44ce9c4 |
| font-types | 0.12.7 | MIT OR Apache-2.0 | 835ae9d781464a79026cde387f9cbc650c9a4909d7cf7e658b127e3c35d2f35d |
| fontdb | 0.24.0 | MIT | 2660c5e9157bf76d2db1294e4a9feba604ef610819a3b591088d0d8392a3290f |
| gif | 0.14.2 | MIT OR Apache-2.0 | ee8cfcc411d9adbbaba82fb72661cc1bcca13e8bba98b364e62b2dba8f960159 |
| harfrust | 0.12.0 | MIT | c03d949a14aa089bbb282f7dd76a498a7f684428e4257202efc119ec010376f9 |
| hybrid-array | 0.4.15 | MIT OR Apache-2.0 | 27f864f10dfb56725ce5ce5472bc52252c8f93a4ab86327122cebf62c5f59a17 |
| icy_sixel | 0.1.3 | MIT (retained package LICENSE) | ccc0a9c4770bc47b0a933256a496cfb8b6531f753ea9bccb19c6dff0ff7273fc |
| image | 0.25.10 | MIT OR Apache-2.0 | 85ab80394333c02fe689eaf900ab500fbd0c2213da414687ebf995a65d5a6104 |
| image-webp | 0.2.4 | MIT OR Apache-2.0 | 525e9ff3e1a4be2fbea1fdf0e98686a6d98b4d8f937e1bf7402245af1909e8c3 |
| imagesize | 0.15.0 | MIT | 65b27460c2c92b037f3f94c538ed9a3342f3fdf923606781629ccb35f82d042a |
| kurbo | 0.13.1 | Apache-2.0 OR MIT | 4b60dfc32f652b926df6192e55525b16d186c69d47876c3ead4da5cc9f8450e2 |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | 1fa76a2c86f704bdb222d66965fb3d63269ce38518b83cb0575fca855ebb6316 |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 | bb85c154ba489f01b25c0d36ae69a87e4a1c73a72631fc6c0eb6dde34a73e44b |
| outref | 0.5.2 | MIT | 1a80800c0488c3a21695ea981a54918fbb37abf04f4d0720c453632255e2ff0e |
| pico-args | 0.5.0 | MIT | 5be167a7af36ee22fe3115051bc51f6e6c7054c9348e28deb4f49bd6f705a315 |
| png | 0.18.1 | MIT OR Apache-2.0 | 60769b8b31b2a9f263dae2776c37b1b28ae246943cf719eb6946a1db05128a61 |
| polycool | 0.4.0 | MIT OR Apache-2.0 | 50596ddc09eb5ad5f75cacd40209568e66df71baf86e1499a0e99c4cff12a5a6 |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 | 85eae3c4ed2f50dcfe72643da4befc30deadb458a9b590d720cde2f2b1e97da9 |
| pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 | d55d956fa96f5ec02be2e13af0e20391a5aa83d6a074e3ad368959d0fab299ea |
| quick-error | 2.0.1 | MIT/Apache-2.0 | a993555f31e5a609f617c12db6250dedcac1b0a85076912c436e6fc9b2c8e6a3 |
| rand | 0.8.8 | MIT OR Apache-2.0 | e058c7de0b26af77780c769414d6257830bb240f3c38477dbc2c16e5f54d6d4c |
| rand_chacha | 0.3.1 | MIT OR Apache-2.0 | e6c10a63a0fa32252be49d21e7709d4d4baf8d231c2dbce1eaa8141b9b127d88 |
| rand_core | 0.6.4 | MIT OR Apache-2.0 | ec0be4795e2f6a28069bec0b5ff3e2ac9bafc99e6a9a7dc3547996c5c816922c |
| ratatui-image | 8.1.1 | MIT | 3ecc67e9f7d0ac69e0f712f58b1a9d5a04d8daeeb3628f4d6b67580abb88b7cb |
| read-fonts | 0.41.0 | MIT OR Apache-2.0 | 046a7d674daf459825b32f5062056d6882db0d2f5a479fbd76ccfc870ac18709 |
| resvg | 0.48.1 | Apache-2.0 OR MIT | 67e3803f97b999e80cbf7c6ecdd07a8102204d92e1633cf48783720c521196bd |
| rgb | 0.8.53 | MIT | 47b34b781b31e5d73e9fbc8689c70551fd1ade9a19e3e28cfec8580a79290cc4 |
| roxmltree | 0.21.1 | MIT OR Apache-2.0 | f1964b10c76125c36f8afe190065a4bf9a87bf324842c05701330bba9f1cacbb |
| sha2 | 0.11.1 | MIT OR Apache-2.0 | 47d7069beb7d6ac7b9acd1039986e73443f24234f41074da099d6f994ac9ad19 |
| simplecss | 0.2.2 | Apache-2.0 OR MIT | 7a9c6883ca9c3c7c90e888de77b7a5c849c779d25d74a1269b0218b14e8b136c |
| skrifa | 0.44.0 | MIT OR Apache-2.0 | 819ab7d62b1d3e72d9d9dea5650bac30424f9111364bb94928dbf5ecad1baa68 |
| slotmap | 1.1.1 | Zlib | bdd58c3c93c3d278ca835519292445cb4b0d4dc59ccfdf7ceadaab3f8aeb4038 |
| strict-num | 0.1.1 | MIT | 6637bab7722d379c8b41ba849228d680cc12d0a45ba1fa2b48f2a30577a06731 |
| svgtypes | 0.16.1 | Apache-2.0 OR MIT | 695b5790b3131dafa99b3bbfd25a216edb3d216dad9ca208d4657bfb8f2abc3d |
| tiny-skia | 0.12.0 | BSD-3-Clause | 47ffee5eaaf5527f630fb0e356b90ebdec84d5d18d937c5e440350f88c5a91ea |
| tiny-skia-path | 0.12.0 | BSD-3-Clause | edca365c3faccca67d06593c5980fa6c57687de727a03131735bb85f01fdeeb9 |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT | fd3ca314f692efd6c868f8408f53fe444634a845f96c028b97d35f6a1f79f0ee |
| typenum | 1.20.1 | MIT OR Apache-2.0 | b6f5e870be6c3b371b77fe0ee0bafb859fa4964b4404c27de1d380043c4dda20 |
| unicode-bidi | 0.3.18 | MIT OR Apache-2.0 | 5c1cb5db39152898a79168971543b1cb5020dff7fe43c8dc468b0885f5e29df5 |
| unicode-script | 0.5.8 | MIT OR Apache-2.0 | 383ad40bb927465ec0ce7720e033cb4ca06912855fc35db31b5755d0de75b1ee |
| unicode-vo | 0.1.0 | MIT/Apache-2.0 | b1d386ff53b415b7fe27b50bb44679e2cc4660272694b7b6f3326d8480823a94 |
| usvg | 0.48.1 | Apache-2.0 OR MIT | 977d0a4abdef933f424a99fe09f95576e089b90aebc6f016a3bc813762493e91 |
| vsimd | 0.8.0 | MIT | 5c3082ca00d5a5ef149bb8b555a72ae84c9c59f7250f013ac822ac2e49b19c64 |
| weezl | 0.1.12 | MIT OR Apache-2.0 | a28ac98ddc8b9274cb41bb4d9d4d5c425b6020c50c46f25559911905610b4a88 |
| windows | 0.58.0 | MIT OR Apache-2.0 | dd04d41d93c4992d421894c18c8b43496aa748dd4c081bac0dc93eb0489272b6 |
| windows-core | 0.58.0 | MIT OR Apache-2.0 | 6ba6d44ec8c2591c134257ce647b7ea6b20335bf6379a27dac5f1641fcf59f99 |
| windows-implement | 0.58.0 | MIT OR Apache-2.0 | 2bbd5b46c938e506ecbce286b6628a02171d56153ba733b6c741fc627ec9579b |
| windows-interface | 0.58.0 | MIT OR Apache-2.0 | 053c4c462dc91d3b1504c6fe5a726dd15e216ba718e84a0e46a88fbe5ded3515 |
| windows-result | 0.2.0 | MIT OR Apache-2.0 | 1d1043d8214f791817bab27572aaa8af63732e11bf84aa21a45a78d6c317ae0e |
| windows-strings | 0.1.0 | MIT OR Apache-2.0 | 4cd9b125c486025df0eabcb585e62173c6c9eddcec5d117d3b6e8c30e2ee4d10 |
| xmlwriter | 0.1.0 | MIT | ec7a2a501ed189703dba8b08142f057e887dfc4b2cc4db2d343ac6376ba3e0b9 |
| zune-core | 0.5.3 | MIT OR Apache-2.0 OR Zlib | d56377fd46368984a170bc5aac5567e52ca5da874caa60bea39fcbca78fb658b |
| zune-jpeg | 0.5.15 | MIT OR Apache-2.0 OR Zlib | 27bc9d5b815bc103f142aa054f561d9187d191692ec7c2d1e2b4737f8dbd7296 |
