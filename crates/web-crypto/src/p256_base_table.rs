
use crate::{BigUInt, P256Point};

pub fn p256_base_table_baked() -> Vec<P256Point> {
    let mut t = Vec::with_capacity(256);
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "7cf27b188d034f7e8a52380304b51ac3c08969e277f21b35a60b48fc47669978",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "07775510db8ed040293d9ac69f7430dbba7dade63ce982299e04b79d227873d1",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "e2534a3532d08fbba02dde659ee62bd0031fe2db785596ef509302446b030852",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "e0f1575a4c633cc719dfee5fda862d764efc96c3f30ee0055c42c23f184ed8c6",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "62d9779dbee9b0534042742d3ab54cadc1d238980fce97dbb4dd9dc1db6fb393",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "ad5accbd91e9d8244ff15d771167cee0a2ed51f6bbe76a78da540a6a0f09957e",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "76a94d138a6b41858b821c629836315fcd28392eff6ca038a5eb4787e1277c6e",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "a985fe61341f260e6cb0a1b5e11e87208599a0040fc78baa0e9ddd724b8c5110",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "2377c7d690a242ca6c45074e8ea5beefaa557fd5b68371d9d1475bd52a7ed0e1",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "47a13fb98413a4393f8d90e9bf901b7e6658a6cdecf46716e7c067b1ddb8d2b2",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "0a0643fb8fcc14def67a6a5eb1bf8e9125b35edc7338d816aa4110a6b90ee785",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "553438324a9e7955c520dacda2920e700da10d00e7012ed7bac0d100861f9cc2",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "ae3f7dba0bde8b6ad7ce2f8eede4b762c556dea678f859626a9e6235a674c4f6",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "1c0549fc0a69995a24b8c213249db9a97940500d085f8a5c1ee0553e711f4b53",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "34a2d4a3b009165987ffd1528603ed61190d0b710d6a564c2db2e35f12d0441b",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "beaaed6a53a1e3c22bca71046e777fc0e7d766b9deddd81db424e7845e93b146",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "04c490528be759e4e8897bbd818d459aa416b9ae0b3c5dfc3469cea39f3f98de",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "30e50b46405cc74fade84c66242a8107471d9d7b4a4605eeebd949434e8d6e96",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "16949b7287d4f481897299b9eb6fe80ccdc5849ae1d527e280e76bb98e61ca07",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "e7a4146d770ededebdf997b75b6012094d2d6ac6fedf983aa09c2be7a0420427",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "b01a67f716475f72886a8f4749b86176281b195ff46f925ae1404a861abe45c0",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "7c521bbf5a3956e2acd56b13164ef99472fc676341b62c7fc72f0dbe0090106c",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "e716aed2cf069e4d997789672e6d6bd2508676f2f4fd0a64f077e8daa245573f",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "353663e694fc72ab5912b06687b9a851d13d0df2fa07c9b3505fc26b469218d1",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "5a57c3e3548207ef2f4541cf25b5e81b6b2e5d2fcec451f4b5113c2b357174b3",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "077c7b303e69224601b1165729f3443328886700134bb92888cf5959383437e3",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "5ce96505eef7208cd10920adecfb86ad1a87a974797e4fc58937f00facb006ff",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d16b6ebd381f5ad5ec56d1f22fde7bc3669d20bebd955c2b818dcda107152613",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "71a5bd27c625ca1b4b2a3a4635b1043363a5b8311d31a91915261b05d0b79fbc",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "c5bf524d066741ffb39452ca1a02f2efc76b6051ebaa32d0c2e37ccc8f277f6b",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "a018366f4e91e90d8e5c643340e586b4714ab749c9052a0503e8465c6eade3c4",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "e2bbec1714110b167c6ce578349d8369d5f7284e44614f37f45c42026b26e8d0",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "cd7ce65d707da2a69ea25487ea254c45eb7d5adffe1ce3eefebccee647a9112d",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "85e04a4b4c966f9789b9016aa5ee6708a190aa498bee60a5ffbabe0d3a92b3af",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "0f5f0aea099634718e67d4c73705ae59e8af0bc36c998bef6c88ad96c3212f1a",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "f8a8b2b7c8593c955886aadd4ba16b784f5ed40dde29af7c755d7b1e7712b09a",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "471ad0a8d57f5939f15f0d870ae25c9091428bf4e1e6e9172435158cfc4471bb",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "2234ca62778e8e392a367ab48094d138b0d7e9ff7f23982c454318fa49693a0c",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "0ec73885141fe54ffef6a0b570cd98d530e431c1aad5fcfe8f7dcecb7d96dff1",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d6224f4e87ae875d91acc4ef580652511d5264ce87ed78aa9ec841ac7c7b552c",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "477778364e6e1cde059a0e2532c8f0c69ba12b3112fb6dcfb183b5399660b7c9",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "2dd89d00b8389e317373efb6a7c89e8a5e0604c1f142ebcf5c205fa39918c3df",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "353d458d3491b5a8c73d989017e14dd22ec38faf35f45d260fd8013b26d69d8e",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "9c7d37bf4f5bde243130fde40c040a8a825dfc12fb71a9a8842b9d0020f32b82",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "ddad6bf0172e2240d57180ff0b1ba76e28e6bf035dc617ccaf0d4e79e330c098",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "0b1371e6935a21897530a4736125addafa333bad2db9d1403e5a994d63b67b58",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "f8f5dccf4c6a93d7a4a54daafaa3449aa87a8069875405d43725c5dce392d805",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "e58176cf66d63054389d3e336461327351f3da64a52143ba026619516cda02fa",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "a118b0f24bc45d9c9b317fe248af581ffd42d88478182a90aca7f0b2ac51d8a6",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "698b53fd0f7e1798df7b25ae09464d6a81922408e376967d8621b55d5c87b030",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "fcc8ca2e4e502d2ede9ec29566d715ea7bd24be788828675fa42e8729cf5250e",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "30b57bcceef8bd04f6b9880a8b34da5c9046bc05c03b2120602e0fbf730fd4a2",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "24bbe05bfa35ddc0ee9a4b43cf2e4e3af349f675d2af3c3d2318aa0489a95e03",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d49405ee090a22cf17182d4dd2760faad40aa52e375aa14987df45ed8d793c4d",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "6d28b6bffd4daf313f85eaad8e4d71b91ca631161f99218f984a23176f922dbd",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "af39d905141dd2fa40fbe1a61ccb4a1c4c24e9f0b84da29944abff02a0ef3cff",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "d131e661dd93d815e613947f2d302f2f50ea585cafb752509789404fc4fb240a",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "34342c8461a95da2e79b4c37f48f4fd47423d4ebcdeacdf422f60e1402501a57",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "afff5af92c807615712e02810be246a2d0434584477fe2e867e5fdd7b8087dde",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "659f6ea22cd060c2099a747ee10a2a22aacf175292ddabe7178346acd44eb3a2",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "8437a7cab8f3e67c05b8000a73853a82e89dfdb368bc666999fb69a100371f53",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "968fd75882812a40735483f43819bb8e10e124fedbfbe259248872d7879cbbab",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "7fe36b40af22af8921656b32262c71da1ab919365c65dfb63a5a9e22185a5943",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "e697d45825b636249f09f40407dca6f174b3d5867b8af212d50d152c699ca101",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "61779471478643436554caa343adfa5aa1dd10b8a60e47c0336dd1e7c68278c2",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "4ecee7d5a568791f03bff005c8986473327969992317a1a69d03ecb2efabd2cf",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "521cf0cd89729d1a193d8758abf96019f7221aba0101b56baeab93965a7d9344",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d60220dacbf9e9bac583fe0ed386371b7a1341ca5a95fba14529f28196b7064a",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "176c11c1328ed07b562fc9efce42700bbd4bbdec1543d1a70b3184a32b3fce8c",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d17f8dae934b8a3589592180b7de3b7e5c5d842a5bb51ab16ea68436a3c9f19f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "6965b6384d7061e685371fe7ff26519e76bba9dda2aea7817aff4fe1fdde3445",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d1bcdae39c482511325e496638732dbef47ce6051a5f99d97cc4389e18855113",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "9ba1e12f974ac9f39ba834fcb184097148526336679189f10375c0c6edee30ee",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "03d2f132311bea6c277c3eea86a877be62067ebb8c7d4f388a96b83b67b43c5e",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "c90ea1fbe9020055485b544d9567e8635a1658f6d17c5b108afbc67f51c57125",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "05aca66c710ede6b19667af636e660b1b6a3d07d6de19a69c669a504defb8d94",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "0a88b8f77c8962303d3a1bb4a48c105b9cf25089f822df8fc1bf1f2dab3cd7fe",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "15b63b6905525ac21d4d29d246caa9900c491c0994d7154c6a3bd234f1661fd1",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "0fbc341c8c669d7632ca9f0d41bc43dc1efe47b273b95775258fca6c4d9aefbd",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "bd8022632f360e3fe4014b1d4957d3d3950b069e200a9ff1ed3b6ea9d3e71ca0",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "829019319d8f776a2dcb6c352620546c54dbeefa7c1e27f7f3a6db8a73e44b54",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "e6a26e713ea8fa1d65888d6eac1dd9a09f47a0a1011354846803f5e4fe5fe6ac",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "c48dbfa12299cf2a444cd57db517ac92b41ca518e198a281a5e743ceea394a20",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "bc83cba00e1380ba64ff2e411bdbd6cd9440b125991bb5e24a0580dbea77a75b",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "987f256d58cff9373be71969fc3a9301e8f9257ae57fdc00cd013f88b049e7cd",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "8e92695694ec505ce860ebd60007e39e47b4605207aaffdbb7254bbc6efa35d6",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "6608c243773c85dcbc666b9ba97323b234a8bfe70a2e3338c6e3197aa3fe67b2",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "a1a916bec521c168846f640da746e03fc22b159f40c543081923fe5ce5b47a28",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "70c1fd6918000aa99979bc856e976e5358bc6e018be062865fc1209b0378bc7f",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "f7daabc20d5bc522aaedf6f122428543f5a9f32f61e1cb0d6d68171ca6aeb856",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "30ac8e52250d76b2c0733fdeacec50aafe32c900bfb0192cbc63b9f4e77e03c2",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d694b530c099785e69aa6584002e77a06704ba32cd8786dca11914fed1dc3f83",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "5bb99589230a0df5ef5a97ae7339621236bba6c05ecdb6af2d86a63c3e689186",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "189e83e977ed31595724d461a33c413e92643ed05bc1dcc4edfb310d1be3a018",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "d8de765227b7873763de93c34d8b561cf73a835fbb8e9c71c2ebaf8017e55104",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "2fd29465be13f2d15c4c628ad1dcf924c2f73fce1940db1b2a02ef80e52e08cd",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "d03eb26a9a38b79b6020f71756a0a320e9227bab9467b68f06eb9409f722a81b",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "50c5fcb02c21a3e76ce057045262d4129dff1a286df208f595e25f86d8a85767",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "fe4091010eb8cda72d3ad94e07c7976fff6986d6f1790dd7e834110d03f61c13",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "081759c1da40403239486faf093b0b79f986fbdb99e1d973c3d3cecdbda0b3ae",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "c90e300839dd58951e80957063154403cb4e6644c774364813d00c248fa8ee41",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "62f504176d19e73c9c0710c6538403625045534717accd6e47a63667c3d7c1ae",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "54ccc9415026d73f20a845b72a58e5b18bd27f198542a0beeea6bc92071e5c83",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "1c433f45b45145323a8f8715dad2bf22929e0bcc5d8ee496cfd08ef7140916a1",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "29e34b1bed8aa8149d841014dcfbe83383fd5b1e946f64b2831bb80c01287c25",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "ea4397df95eeb3a3c1417cdfccd96b25cf3a198b6e15a4391d23ad1d75a7e46e",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "d2a076367c5ba19eea5b5f1d3c001919876f31533ab62409f3d5be3a53a3a383",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "79aa09b893b7fcc06643a0f5bb9dce305981aeff3f598bce65f2abb22e7e0640",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "c56dd686b0fd846a66dceb631977d5bad505671ae1abb34a7748532d4fa43529",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "f2acaa85f3f1b263555d2c60a76b8c11619e467816fb7eee59ededacf9a521f6",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "c5440c597814a47d9f6cc7d1513d7f38e40cd02e32847f01c30fb77122d32936",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d27ee9ba383e1fa72edc1e1d23ecf4acd8a6d28631e41e5d9a42fa3747cabfd4",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "d07e95892daa15afff3e42d52f5bab12f165803dfbd492c0cbe1a18d2c2e32b9",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "774e4d6506724528731f0769f893ef4e23ed98f15228e81e5e78ad6a2c1a4f5c",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "437f6e07b86fd8fc3a06ae0295829d324275a149e19d957c7601aeeef79f7b91",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "5fe8d37ca889f13abf38b8289b15d63c589a77119139683364b6ace375270033",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "cecdff7a5cab844f79ca1e2a6483ee28b80a7c8a3447db8fa969ecfec51cd9c7",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "9323e54d2286800892af68819c7d0f6b0d09d06b645e5a3feb76eac729389632",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "241c567a4227f1c506c79b97a6badca61c37101cb8971583bf9f4172b066fd48",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "40a62d93d4302d4b9363817c043203a48ea87138aea366057f7d2c6792857b08",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "4ea220c78bed1c8d609077a9243b173ca518b52ea200446642ae5b50783dfb7c",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "c644dd2f1ec88d3a27417d9d954c829828c2a2b452324480e5d9e6e88415e13e",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "9c8651ced75ae94eeb27e145f21c1935558dd9397551ce0d0f02e89487d70d2e",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "9532e363ad04e53e3bb36e293a2454c8f3ed01a82d75cf129cd3da9320f35d28",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "298a401a6ee92414a708fbc0a52e5a71e93e7ca26bc0ec3617a4f678c5d62af3",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "18f9a3aa04fe0466e40985ffc80c90df4866e6b9604ed6ef94763ef0af0a3d96",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "0fa822bc2811aaa58492592e326e25de29493baaad651f7e90e75cb48e14db63",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "bff44ae8f5dba80d6f4ad4bcb3df188b34b1a65050fe82f5e41124545f462ee7",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "031a8747df8dc746e4c13d030696080153fe448a57324591794a16baa05f57b5",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "883a2c64fda8d58660e8aa6c1e387a321431c18c42b8def21827ee579c0343fd",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "a7163c2b9b973c17f9571975c0d5934a4eccac6096513cca015e2e65580b2322",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "308a9a797af31fa563389991545a6b7ab841a4f4e09952d73933b2232197ffe9",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "1bffab8c03ab8279811b3923ef4b991f02a50c382db2670c6cb04a6a5c42a280",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "2982b620ceb1d098332ac758529f1aaa982a369aa9d24fb6c9c742364ddb9261",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "54bc18d7a99899547ddc6988d7ee1b3f2b481ab443da43ff68f41305b76a6987",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "4b2c8c1211e6eaf37391b851ba73e2fd52eb8ed4bb73b119fe457cd05b9aae49",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "1f380071781dff16f33d8173a6fb4d96ba770f18355cca4cb2a64c6196260250",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "b1521ec4ba6c681da33705176ca2cf621b8567660590d693b1519b2e6b011955",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "99eba192aadb019f41e0be2b789273a058e6ea55222034f3ea6d95d177ae84da",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "fc4dc4cde1efbfd9f4f806644530184db630d866d7108d108da39dbe02155cac",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "6d50856433968435fc5945ce21621e089d4e993b7604a5fa896bf7c7f7f96003",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "ccf57357e968c6e7c6bc5d8aa2c7eef9d6c0d6dc8263a0d90271ccaaac7e969c",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "1d35c9699761e3f285f248239267756f5194b85279f96b7c60aafad170aae231",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "c7226cb62df608231d660ab622ebf810edf58aa4729a66f15867063accd6ac71",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "6f2b065a7800cdd89910e1c2a6397ab4d9c9823c2b473903501a327426432d92",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "0acfeb77e5e7a2e4eb1472cf700826c5093bf02de33dbecb6d2d1c4963e17c9f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "870ac12ba3dd777468991da7fdadd0af2f3352569662876be3065e088a6edbf5",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "7543728b999a76998de7efd769d05a67d03c64022e5f297e1906d2dcf02f8ce3",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "dbec13e6da0d5592a0bf5fef8034db346a9e8c749fdbf0f07aa6e46350c06af3",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "f859978fd619e5f8c0e57348b745e4d442243a3264edb6647caf0f7cb6d243ed",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "55d9a959844b5aef388ff0f7aa02f29acbf5ca9aa567e0e65572aea8750e4f5f",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "69cb7f9aa5dad203766d574fbcc8ec524c9810c633ad1b15c858eb76bca97db0",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "8c76689ba78a166138f9434c2f72e662f517323e3f09b5d30758cb4d57c6f8fb",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "cd3172990a409d3a288489b0c63d6dcdf0b11fc3136bc05f7dc5b39a9bcf2306",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "3fb5909a93dfe46af0c6a4169281bf8746543768ce5e12441ee45f92ed69f1d5",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "549991ac0be8809f704a4e3a3bdf775425d35688fd0562b013d4fefe99a56cc5",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "c34ace1ce5dd90408ae227e16feaf2537feabb0cbeed3e03506f1270bde7bc7f",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "e1ef3b3a009857e2e623b8be80166f12d6554299bd55a442aeb63980697806c4",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "6e29f959be28c47fae5abca185755c08346924376f5412c1d4d3d2de4351964c",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "34565d9f500f32f65052ec6cc184246def640c527a0bfb63118824bd563fd88f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "8efa0f79b5909a0b10cfa381496e68943b09f0776ed90cad61e0f0efe5e84da4",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "242418e7934cd6139fa46f1c5daafd1479957c4897816035fde19f42596969d3",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "d0d4af73cb8fdb30b9d6d6d0f360db2f1f8973e31eb3f3974acaf2c0023ac4ce",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "ed90f8c8f15d7ddcbbe9ccee3e52953061850b1c863aae77e3df0360a47dd6b9",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "829b8dd0ed5452e8a6d6f71b65b85f9038d1d60251256eace399da6192397d42",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d300ec543fccf46d99e73c8a896241af5c5adc4263ad1a5d91b71631e3df5612",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "ff046a9eb2bfeed9c00f2ef0796f458ec141c259a845631128a7d4110cb71280",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "432f55acc0953a170a01eddbdd4cfcc9012b6e6ebd28487a7ec3271f5ec33919",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "5890c0f334ddc2b08fabbccc41287df4011745f856de35acb2bd41c7b0b3da07",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "cc9eac788b1d9c045e4604fa4f79272655a279ce3e2a8166623d0ae88d3813e0",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "6eec95670d54650cc14b66dd02436893ffdc67942d666817fdc73e83bf780c2c",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "14bb5350997732c2fc281de065ea010579ab66153a07ff89089ec1a1edbfcd32",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "a0e2410cd09c01a1b79470f6ad3da82908f69591eeb51bd853d6d6e9019fc555",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "1e29648c71bfded42ac3b57ba5b2fe898bbaf19f6b667930cf340818c1ac6d8d",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "e486c7dffeabb058c1f9aa2349ee7eff8b2c7e63cf570cc5c7d0b24cc5852e50",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "51fd75ed5606a12e9ee88a5c9f51e05a694463d63392ebd866ba3cadaecf107d",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "69a16e245a5fcb8821ff1441906276e963ad9e86c339d0ea74502ca378692e20",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "a7c968cd891afc0106f01e9a4cddba10113d28620f7da894996e69a9b3116c8f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "dcad8b2a7eefb2c721190e12fa0e745a7a39f95bc79b0d1747bde79706bc8d71",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "8bf1287ab91faf9b2211993ce07c0fa32726c4077b717b56470cfe2ea297fcf2",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "d9f8b3771d318bc54dca9848228c57c413fea55aad0e2f87d7273629aa99ad56",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "07aad641f0d22377c6551557fb70a4a3995de59a31f63cb4d4e919385f5c61b9",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "f41d7f4bb5e50430cfe08cf8b5e2ee0ab63b9998a43d1fc584b6b8ec2b519178",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "e6a669bebd9af8d6f1046de3faa82347ab49acc36919e1f9a7a1665dca6a3551",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "3b2369ff19ddd591f85bc3da3881ec450d80e91bd6616f3d2a937e108f089b4b",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "5df102ecb29a762c2d22213dd27d78763970e7b02723e7c1f065556f67f00f4f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "eca3eef00455b406c7661093249bff8f997cd06e999c1020bb180bc7b8352aa1",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "9e6bc281e78c25b8f6b863f37ca79007a8359b399b78ed2385f3ad39ecc12ea3",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "5c97699d44b7daffcba214b7fb45ea98468f41ce1b0dad7bfd94a8b61a93d03f",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "4737a353a4cb0fb1238ff9a6ca28f2ec20680f327453615967499ca90e6f2ba7",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "4a5b506612a677a657880b3a18a2e902e9a521b074ca0141a84aa9397512218e",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "eb13461ceac089f1c42604fbe1627d40626db15419e26d9d0beada7a4c4f3840",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "cc8fe9ecccdaf543521473143eb337c5da74be4d8455b67c1b68edccbf8d5842",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "c2a5d01cd9f2fc3f75cec6140b91fb775df1592d458677c8567e1ac9487100c7",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "11c881390823d8ce8006b50f37e8626171b0b96cbc1184de23f389be0c37da54",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "82f5ca516726bda3a42aaf13caf61ae4b3e32b21384d2adf567c651408ee3df5",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "9c7a4efa52a32680676e37e7fc8b8d71cb4cb3a72a5c98ea355c95a1fd698f73",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "a2c4a71ab2150c7b16c424c1d0ade69b94b07cd5ea879e7c122443bd120cf142",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "2eb3910bde2ab995012c29df8bbe0f5032c3b2574328e5f76628d837008e2df0",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "3f29c02337474b3a77d37d348da4999f9540d120ecf65f490910cc4ed274eaae",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "d32e851a25c965c2fba2f5bdf8f0482610ec719690525db1fe39d9aca1dc428e",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "6bec05a1fcc5d64e45cc6d22a98a834031f58d904694f768b551d5b501cfe158",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "e644041c2ed2cff4f54d0746449c0331c681312779800dbf71fc28e6e42f662d",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "bc79ffb9de665acf4ba0050d00c7b4d052e1678f917fd7c06711c11d66e6d591",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "a626c3f311cb5315b59f52272ca6064b4c287dfb6f6e6aaddd030872b7314c65",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "bb882a6b990ec04cd949b781e9efc624ee365bd68035523ab6607db5a63f1919",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "7ef2ee3c5c792a0c0fef6335224d9428a7d2c98f6743333ec739a5ea3ecca7e0",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "afb6862730acc011a4f67f51d5e609db81b21450dfbd3d20302b22dd552ac094",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "0646472486e68db193539def6986c76b5e880be50c09223c6db02c92ab896d0f",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "f1f4ee22fe7063bbf2b3f5cef9047cfedb1b895dc63b4f9090d74508a4f528ea",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "0d42e1f15292fc712ea713544ff03fd17beb467e384d570b699c584da8d5825e",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "adda2b0a21f69acb2c1f17a55a85ca7e0c4cb2ec4b4340cbed1808977a8946b3",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "44dc4d9e945c7db20e19d0c28f9ae7518c3b0da6f68440e7f9c18a469ad780b2",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "dbeb918745c2f9c9f8fd19dc683f5f95b47775e1c2cec39d91437413ab93b11e",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "0e51416421640aeb57802554eb5fa77aa9cb53529975e04a6701f090ec49e853",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "cf331cea65905469278eb4a53a91030df568394190c9ee36336e3d1376405cb2",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "8525d0b3ef76314283d8640f8db563d543e17e0a2710f1a2bf7fbadb5ab7aa65",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "0e63a7e111f91fd606cc431c1e676976d0dfccc1923f092af9be830fdde8648c",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "1b3e00b320971112b39e7d7665bc22d4d8432893c3d8180f29dd16ed044c1b21",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "9b442543faf3e2d2f11325d9c0df76844fff41a9f0b6a9e98de24014b7f594c7",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "778468f5252b48e4899db93959f2e87626ddc475bbfa75b7c828bbee5ab90065",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "43341ab10eb39ca06f65136a7f01e2e927bbcaef00acdaafc8f76aa2f2d79ff1",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "224a02299eecc99a0634a786f116445779c3bb5b5501d267f0699bf9e2f2b734",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "fa41a8d29b6d22b4149a08ad871d7fff07b704b673c7afd0840f585491ec7fdf",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "afd35b35bea5bda1760f78b8af6b2261e310d10e196e11dea8df3dae1991e607",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "03555f48e204e99496d8ee266b81ec93bab7d9872155af415e0b73bc4308126f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "e5ceac4035eb4955a0c7cc97b56175d505174da42f93e68f7e16cc51f0b9a00a",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "660cb9301fcd56b9854a459c3396d1e48ceddb7586fe794f157ff80e67984a57",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "0e415c462be679028ec64fd2a2b75f9c944995a27d329c764f556db22343750b",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "c3056e7177f984e227bca21077f8cddc6ef691a45ff7ec06a6097c15becd56cc",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "4a89a61457374b4ccdb5d111b56493fe167d5ee4d1041ce2ea6065ad7789b84d",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "45b04e87ed480d5c08c2ea979fe677e6a72e280634711999ef9b7d7f018e3ea8",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "55b66e6dc586296f8aa4bba0ae54ddc73bdbbc488e7cdb0820049d0364e11f7e",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "9043a91f1dd1419bc386cc04cca223ebccfe610621b594f904bddc671c77d076",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "db3ebc91cdcb79a8ab1d0013a1821a0b06614d6a97448f012046400ecc4bb506",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "a95a64538a676683af1a685e85539258b21826f5e8f7d202cbdd181f49b3e60a",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "ded37dd152738517b92174e3aa64a3bfddfa9e91c66ab7024f5c2279ade5d1b1",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "b14e3c5a84e78c402b3caab3824544b8f43b076d5912844881cc777712c778bb",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "e5e892363a31885cedc2f995f36d1f90ba82337d0b1fc80d3438c84a72bd05a0",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "77439de4da1b87d21301eb01e79b5c3eca73995b9cd099aec936de2be1a69f5d",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "c827ed763ffec8d144d95fecd3177c78fe6784fa0f696a510c665cf973b4df41",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "1e4c4475f2da5ed560e17d7b66ef3cd3bfe9a6106c0047b32e5b6a2380f91f4a",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "b511001491b7465725ed93eeb3c1587d8998961b956e7dc2405904b346340d60",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "9c3981f431fe4c4672ab899ef769f9788fb582b11859479168cd6178653ecaa6",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "60aacaeec40e84091cb2f03148e4774ec12883ed0e4c688fc193d479678e27d3",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "9677dffcc5def66dda54f7c803de04ee46d400783a680f4bff7c610228c4d2de",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "e4107e431e221f5071e7474104fa90a45312f0c21fb084de21579992fab5c2cf",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "1e5f11e6cf701c9a3948c668741c2b323c7892dcff7b2410d028403f2b955c2b",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "2d5295db41e6f839a757f11164d63319a0654d2666872435510f99b4549e7a36",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "f98acf28068072323af16cac763b38c6ea7b44f99730ba3bddb68195ce32eb67",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "3877152e0bbff37eff26ec4601bffcda32242bc07eb98697cfed817743a29747",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "cc70a7f9014bc61abe16116cb99d37309fbd1f6ecffee641ba573e25f24d4637",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "437a6a6e40d04569a67834270a8e16495e3bffbd135ea79f59469a0606bba654",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "c2911e15587f2492ab24825ff34dc5a014a7c3761938d4103755406037a7c0f3",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "447d739beedb5e67fb982fd588c6766efc35ff7dc297eac357c84fc9d789bd85",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "2d4825ab834131eee12e9d953a4aaff73d349b95a7fae5000c7e33c972e25b32",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "a263919b4945a1d447501f2a3c0804c3802f779ea7f6803aeb0421211a6b665e",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "873200bd2aed20fc2e9d3c9de60d60c5ac3f83df4c00efe29ee4040030bcdcfb",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "2890d721e57e196118e63add579547f0acad16e63be0aea8f6f1d3ac4d771f0c",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "69b5b8159ddc032aaf77e1d1416752ec7dc0e7f77ef540690a5728ecb5890d78",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "7b8b8867dd4d9c6f81690628033de305d4e3ff5e71da17814f5822efac7e9e44",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "8bc6273e511680b42246073c7dd5adb564057e6dbbd723650767022aa8cd12c3",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "9022e314949ccf3e8937542b8cdec18ea2f8d5618688ce241ebd8bac137de736",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "2fae5e4f2904a39466d0bb045226ce087f49366c44ea7657f4ef5c0844c42ecc",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "a77663f5fcdf189cc2731f7801b1133ee130a96964396297872e52cf757a603c",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "4e139cbdaddafc58dae982fa5476a35a8ab86c568cbace48d91745804ca8e468",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "1a7098d2db889a1112911d2965f2870ea93d696bf79b5582e83e2130461dffe4",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "39d474f5ee8b69fae5a42c936f76f32768682783de26b553f3ba5eed161315da",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "ffd92260ea330bb7ac43ff1f15417fe91a078f4b263234f284100f6f2472c9ae",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "cf4b93b0ba2111475999633225711e5f263d2e9c868b8a34a0cec1d866d03ae4",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "73baff0419eda72389386cf2b5156deddc34c10619515eb5741145c144cd3397",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "1e97de634977ac5f6a00d1e8c18f825e779ef92cf826134941ef2a139adcb8e4",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "6fb5456391ba430536ca35ada0f4871e140b55dc318d93bcacc1fd7cdfd83618",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "f663d87766c6b8a3ed3b68c52109daba2f1e6499aae5d33e3f39f5cb02430626",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "d554ded0b2862113a145c7ebdf98e4b942d91e70b46ef479357bdb041cbd9a71",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "74b8f94b6c36708eff0f7ad45ac393145f29fea9a7a61c99a43a4a8b3c8cc32e",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "b889d78bcf05f1c763c4e454296251719f61776622130485de82817f3be28c8a",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "babb0b07fa511b9b182be4a975c700eadff631e5782fb8c33037fe51eafdf89e",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "9cf646b91a4c25bbc974446c2976fb982683bec78b098cb30e2e5fb31fa4e33c",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "37b0624dc1f65a891e408e258b821f319e205827ebc1603219c45e060e0d4563",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "fe251903a452329835917f8d5547844f6cae5469245e15f58c496939c224ef48",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "4fac9cac97ca9be0d836b9405a1b36a0455aaaa1c321e408ae6960a05136dfa7",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "cc411e541368f9f2e2c3b6c3f6131016cec29f6bbdd3e7a5bccf5cb64d405a74",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "fecb812fac774584571527fc350186d0be4fc2009f16fb0d645be6b618d1bddb",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "8c6c923cf1a4351cca356ef3efc35de10898022a45c4a1b196282910aee1337b",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "a1256fc84d40ff6a5efbb2319eaa8e84845362c1c6ddfa2acb451d5cd9c84abe",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "f81f5be38b8ca534e8a5c7938d18df9e0d238966f74e1a6b826fadc0523b716d",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "dc7f49329c1f06df1d56d29d380e0328660758503a878330464002f512632401",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "7459e772c29c4c3f158ea76280d631820e4e1a713bd0572546e0c52790789539",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "892bc7f5183444eaec7e8e858a2ec1a9383af5e04f7db3913cdf5a471439d97f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "2e1c7c383d7f8492c604c9f87d2efd3f345e4fd95b8881c4f4727e8c8ef2dfe7",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "c629dc2b09f0be266d9f4529ff7ea8f1c2d747d667b0c5bd3f85cca2c55e5f2a",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "8c86799b36e7d519e63138a90228dc28022fc19d8429b019acebe833f2f21873",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "5c0c2c12a5a3e85d61ce9fe475a272d15dbf4306e861f0ec8905b7d7e5159140",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "85685474d77e08482397f463e53cde1d022eca56c3915c978ed9c7e787354b7a",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "20b50eb50bf587b6eebc913b2a6f728706a891dc1fd6fdbd8954402bd16e04c6",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "80ecab9f02b4cc6c691ec77b5af1694c45faf0843c5344c1186c5ae0464345e7",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "f424148aca6d55c6fb5fc8f83e19fc8432e500ca5bccdbb497a3de52e042e3ef",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "e15d0f2321cecf4433f1c0b1843eff4f5d139a212f6bc03d2540306574362bd4",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "9a84cbe2707ec22ec1bfe039308be32623ed4bf8a4468269be6accdaafb46fb8",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "e7a38889ddb5553d58f94187dc6d34dc5b4273b92840cc66d376b93b369f9c65",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "5437b5bab121917208e37b26192145bd58a22b94187d1ab73b1be4e0aca7ee5c",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "1136b759c12b3b11b319e52d6bf9597d9e3607554615622addc6dc1b12378c16",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "7dec0fcf45168fdd09f0abc60fe9ecb29614aa28e751cce379f59bd0b3488127",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "11f1f5738159984d086f8c11b6445072d95c410a18d0145fd0e6dafff34d3977",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "ab98c1b579c0b985cb829f87d3a748c7937e953f3dc6820614bc90ea00154b71",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "934c89b9aa1cdc489b8018b75e7d2586953b9a779a2e613fbe840ca84d6a6a96",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "856af4ffbccaf40a4516d6d6b31b2e9d107d2df0e2e4ca9febef148533d838ed",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "4d00885c9a9df2158f5832665cda09c3a8030f018d3a3511febf5ea4acc506c8",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "3f853c3a9671c7f214c0474409fe88a145d17acaa56ce133d4fa660e1835aa2c",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "0d2bf28ba7c2a51a90f573a82589f18e07e50ab01786df709c762ef1943e832a",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "0cac3f4313bd00ac7087a10a94b4e7ed27ec9db96055144648263af15b20d37c",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "84feedc3620ebad8fbe9218bf89db7610b63f06c8f9b6856fe43bd70a38305bd",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "f199bbd5a9c7c1654a3f5b5c17da39482568e12ddb32c53bfff41ff86073bba9",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "5d890886cb475bd421501871dfd7fdeb3c399f34f03e03f0582554a2a1faaf83",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "970938bd84a2f69a46b14def6cb0616d755b7179214633d2498413ff2ce31fba",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "a6e1c5814699e69cf5d74845410e16291bc3cafc1640d35e0604c5b8bdff8874",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "0c25ae34dfa827e624b1e0a9ef5a2efdb789999f3812b2141885d8ef93076daf",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "8a535f566ec73617f5622df4373713269e4c35874afdf43aaee9c75df7f82f2a",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "0455c08468b08bd737e02819085a92bfcde533864c8c7669c5f9a0ac223094b7",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "4eafa17a88814d84d36e2c6048bdfa175799c8f19419149be8b043c401cd7f36",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "a47974d851ec201fee45531527efc47b5036a58cad4543d2c778c64a0b2d1c43",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "c6122fc44fbfdb25cda9e496cdf1c58971dc987a8febc765cc319d54b96214fb",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "ea205bec8343ac419031c9868877f91f08a6e48c1e271f8a3702bd8fb7adfff7",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "b6d94022c181213724b4042c13019fda6f4fffddee86b987e97be9ed0e9d6b84",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "93ff3f41629287238634478884d77ad3c23273eaec4f382ae1bcaa0fbca7cb21",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "ad6090dfdcb41a019af7cd273da8a366b7a3030e99e53d5e9c837f4fe476c81d",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "77b5d1dd48f4c91eaccd75c2cc5bd8a8393fdc364f3048df818abc84ee6705dd",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "7e4573da81bc84dc2626f6ca66f6a11f227f1d7586bd4cb5ad180dc0caed764b",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "8cbbeffd96069f25f1bb9fc93651b20b2fedf3c7d11e7fb49c83115f17c2f2f7",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "af13583cbf65021f7977aac8fc30c25f1a9c655f50187a3db5252b56c3f0e03e",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "f8fe31f81a9b57b3dbf60be46d83d6f8ca20edffa3e1dc84bb2f5bfc575ccb22",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "da2535f6cda744d628e91651abe7add0b2a3a6939881b969142fbc118fc3fcb9",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "e756083c27486df224454c68510dcdd3ed8df5246a0960d978086193902a869f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "b6e06c516305bbc4e466557075b929fcde1adf89f510a848a52f2c3693c4a205",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "bdb4277c21b323f5c5e1f126e79ed73b63f99419a82a9f87537f4d865db36d05",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "a61727a4f5f8ddbfe55703a07bee1e8daf3674c6d797eb1329f315f9fcdcfc44",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "03fd9e4f9b02696ade692d5a2bbae158c1b9010462f080f393b82a7f0ef3d60e",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "27708f23e8bdb74f8c89f34b7cbf6a9e324a7181c96b10c893fa154a553faea4",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "575f186149caa4a4300225d66cc2c973fc6d68b3d1965bd1b21ade75d0802d2f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "7cee417faa66dc6109bfe97cab77ae6cb0844aecb2da65107fb5bf4597445fb2",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "3040efebe1e5b9f596d871e7f9cf0bb3a667e8569ed0e31f4fffcd61a8223a26",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "45a511c97f608bf76dbc4189d991ece618c452b1b42a627f3cd5f4e4a9aa52df",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "73be0ec773ea9b6d3fe3337fcb625ad25a919b27d22955ce7b52bd12125ec16c",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "4cd2a0bab6ace28a2bf7ca0444ab1b75020688b6b3a0e5f213cc521bf003e44a",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "6adf08c371d224056d1310756218d126bde2e7d1c97ea5d64993eca97b19c38d",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "7451fe377b24776b323357a021c84544edc4e459a0829db27ad039a2f521ba40",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "16396435fcc70940d69002198464eade36b09eb9985d5439d0a61cd3e518dcf9",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "058fdc0dfe5dcaa324a33a3bb84ef35ee9d08bab256a19046c47a8cbab3f4576",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "90ee3b9e9ea9e0e7c3df9b4606b8871e1e801392607577593a45e3e1a0110e9e",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "20e118564880c79cb927e3501cf2a53f030b86e3cecac60728cf1ab99076f57b",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "ff67b352ac437ef731bdc3e3fb6de9979fe0dc9b40e1b71e8583bedbada7afe6",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "4eed0b4dfbb5c3073fefe10d9289d72b62df6c0f0ba0405b9e849544d4c20855",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "ace0fb288a56693a1941d3287fc115c81136ccd32d34fe3f4c087a3f8bdffd1f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "42e0437c7aca84f70a45b1168ed12d20feb99f12e6a12de783d637e60524722b",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "2de1da0901d920eebafd84ccc9b4ea3edbea2a6daf54265a766939c81fea76c6",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "070d96ccc51afb00c499ea8fc2ef48c949a7cbf2445cc228c6f24fc37bcec904",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "9b915ef5ff0abc21fc61582d55f0e97753bdec7b34681bacf62f4db2a3ab316f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "5c9cc4f8723a027b0318ec7fdfd7dac93fdd478e694fd54add1452e899273a8f",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "84efe07a1dfed259a827589d3708af964f003675a11acab5addaca695ce80d6e",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "0e3ea757deb82767a824a072e4c20a4b8bd467275c0f80d40f2e4327bcfc91de",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "13f2bf92ffae215623f2abf6056aaab3bfaf0850149c5383145e8c5df582f54c",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "9a6cabe1f8c78b4f154a04034f63c34af6bada2932f50e57c5f483aaa556ed7a",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "ed9f5029fad500451de544fb7d555740feb87eafd12207162521483ea054a72e",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "ed19a56a1b59cf5cd880f08099a2e4dcf318de811d6a8871bcc2887a735a8087",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "cfbe683e6c3b56b630254512ab54afe1a98cfd5aae96da46e56ca0873a9fe557",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "baffd4ebf8a8b5ab46ac18444f3dcde3351a3a75556c6c4ae7dd9ab328523eb3",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "0fa93dc5dbc95e83003a8eb3795ac09c24511ccfbf48b6ee1978300cd5acf387",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "cafb6c8657874a09751a418ae4eb8eeee9817b137d07f1eeae03ad0f9d2157ff",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "2adafb1ed5c5c67a01a1cbb99ab4e2e09c6ab445913b9e0e16cec08173ce8988",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "894890b2fad37ccb9a879a27cf8ea9a337d28a48a4bf795c4700eb8ce51b2cf9",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d46800ff7c5b301188a0abfdfd48e438498e7116a3bc2ecb0d187ac835979d3b",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "463d8e29e481aacda1697678f8b8c55ba26e2a1e94139936bcff60267e460d62",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "400f734b7ae2a66ef1fd6c6724f852968f9eac14da469b0a226650888c11217f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "06ad2a098fced766ed22a34d257e4c96479f0b1a76db3c5f835208d8ba111101",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "2d5b8b1c58f85e3559a4c01bb9eb62a0de429d82b70e89aec1a7ba9d5fb37ef4",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "f657e9015f9855a5eb9acb6ebfe5941a08fe3cda3b36a5dc2b1f0bd0f65bb830",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "8077a8bb3bcaac496dc83cd25da0e7ffc2abba9cf2c8e35ac3b9a74d87b193fb",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "e46eabc9f8e2f20cf0ef002c94979625855719520ec5a6e7db47ffceeb3fa150",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "2532cfc894c18963adb002810c09fc3a50f68cc24c227e4bb934894c4a0b139f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "107a4485aa07fe2e14bb5af4516750f0e06a970759acdc147724967e5833c4be",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "6ea22142b8e4d7cd813794ec48e680074d3a4fc91d760c4a6566ec48c87c80dc",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "a6d39677a78492762736ff8344315fc596439591a3c6b94a6cf20ffb313728be",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "674f84749b0b881666b8babd2d27ecdf824a920c2284059bf2bab833c357f5f4",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "3f06dcd2ce62e80d4a661caa2d2a108675f3b0caaeeadd251583948c5974bb08",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "08bc2550ce1922db985807be931d9b2e831606905e88f53e89655da995083c99",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "ec61ff29918a1d05e0c05b31527e97b7eb3228b94bdc794c1c101b8fd5c7c27c",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "02fa1d3dc1b257a1d0960f82c7759298fe3c46c47690233dadf01167d9ddfe51",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "9a79bfbfe71e347f4d6c6698316797e2f5ac2a3900f5abf0c409332de46e2050",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "e98b4de6d316e200b6f671f3b224efa9ca94faccb6dfde317a3f4781926250d2",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "ac25da80089cf4e033d4db5710ff5936fd683b4d0dab013e6eef62ff4514c6fd",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "ebc69d985cb44c7b883da9312a1b338c810983e8243bf37a60b5397705830541",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "b467cd65660c13b84aec42b5296bc037c1e6b5ea71a0d289b456511069962f3c",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "dc91b9bc4d36ffeabb3f5d2a011664ac3cb212df6cbda472f75584cb22877596",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "92f9a6f93fa37761979280de26da6cde08533abeef2160f4919b1597b43c7e8d",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "bf618d715147afe8ed41d3a6f5e542df7309284ecea312897746055552bf1285",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "5ce4fd836044baf3a1623065bce83b11fd7516a5b209d887d79a7081afaefc6a",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "61042108aa50f3681e31bc1ea832c7144abfbd4ab9e4acce38606dfbbf9db9a6",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "285250edc3bcfcd9027bba12176d343a26035672d5b4a81155d4e37a2fd20bae",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "7866c086f4ad7ea83f7284ad1046c1716e9b1a2cf1eff323f9de455a54e0ab97",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "dbc091d89944b44eb651d0cc325161c03840e563817c6662f0f2efdb220ca9a7",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "092d80eaae8567952d57926d1e4aa5fc6367309fe96192c84d20588316c8e76b",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "9018baa11e938f23b322489c6ee06d88ad3a6dd5f518e8d7d6be4fd07ef7d72d",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "c9c382d871f9c6c9e8c147f743bf00b74eb56d8002736aeb1267044fc036ce68",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "edd513bbfef3fb2463336038135004a3dd2f1e9bff60a8a607596c1f536ba602",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "257b598c3bb3f7a97c592ca290f0188b61854d442da5dcac3b8086f945dbf993",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "c472c1062ed0366f818debc07ef8890e1fc5147dd0dc267c171f3a2a3e07e4f1",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "41820e01945f70e96432ab5a84d667d885f3b7b1e22b5d8a571c683d86b0daa9",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "454e27cbfa01cbe542cddf6fba29971ef8c49096987f88efc0512999fa863437",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "134a4b214cf01e9e9e7e5f8f2f81017c6b8d88d5c651b986179b00e551a6bbe6",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "0ecfef25fbc3698c06226d5629c51fffb94130623c2c03173828179c8084a599",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "8132b98bf295f2c0b2471c839f9a7982ee9d57df992f62b23dcb4c13d520e922",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "f11f5d1ddcbbe3469ec2c0fecc2f9b18075969944b3b7edb644630ba41330c21",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d20045d6570278c2b3e3750f788716610caf2cbd5792771554e06c7749ee7ae0",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "55d5398d1666432b557582c99ad534580101fb06a050e62c320f09c3839bb85f",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "576e229049ff8e2d059c6a9e8ebaa72ad90d6a7f1833d9e1f7f631184fed936f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "419a4f03ae7617ccfd23639585d56f8d28b1eb93fff3c118f642e95b4f491155",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "237c2d343a6e835b2e334b1484804545c7ab4a22074c68894b91ba6ad489bbc7",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "a2157eb3dadf2828372304ad31abf75ec3e61dae0b35d0fcc0841ba9e49e8f34",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "0817697da70df31d0fb42577ccfbe5a2064bbba35b187f4e1a8e983289eebef3",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "dc386db7f409d9e0fb28652a88c376d227fce62d1df14515d4eb281c2d5c502e",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "526c6376cf20188ae7e69c459d7ad72b372c3db3cc760b7b23b47a257bfc1e4f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "884286463ca0872694febab24bfc3383cc268940efec85ecc62dbc9c32513926",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "f778429c01c2dd5a69660db365d81207ce0eb122ebdbc15a0ee4282aafb8a54d",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "789bbeb0199b2c38b48c9ad4808a2e9f0f3d27b94cd4417f005791ea7c2ae635",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "14357e6199189bc68680b79f62f1217a7fef5a9c2ca1550fb781dfecbfe96db6",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "87c1d99c5ead72de2cbc4a513a0a9bbd5156b3fd89b84639b1f11f16f529b1cc",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "7e1654c6985d0bcdb8617a6ca55ef683bb0d0eb2c5b275e3cf26978dd5a94d6c",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "2800043ada0982e22ddb6c3ddc7f43d53719a8b66bb19be6f0f679f10b79847d",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "0f99174667e82909b6c0796584a4dc3ba87058dbb8513ae9fe5b0083908d9eda",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "b4958c4e21de21d47fb60c55edb925559eaa8520a5ed115e3e0b5d5ad4441fbb",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "fcd6e7bf996640efe5c041fbc38cb7e12db9f6132d4638414c7e3c9f4add7118",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "a41e9e9ed5876481e77ceb19f84df2582cdd704dbc1a71c7fc4da1176bd00b59",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "50dc31723b30f2b6da4e1ed1efc739b85cd253563dd4359fe8dd28cdcf2e6f15",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "c02a0f2e94cc35ac2c5f09a87d6886df32a6d94ebf58e793ab0c96ef401403f9",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "8ebaa8a18e491aae0f716d457c5cd473abb913066000adb521f59639a7ed80b0",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "b8d550a10d9265571a765de14913e26b9a6c22e3eca47658a69173875f7a8d33",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "ded908e0ed161c307d1790c97073e188541c3aaf586f670d1777b77bfec3abee",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "d0e0919688e932b64134bf13b346027cd8a7fd0b7d761d04ac24e6436e12e1df",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "069c9a59fe7d1452b4eb60b691dbf867544cc4bd9846b8df6a6544db9fd28fbe",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "49c696ad84a3a2cc66d8c6be3c064abe8f7137ddf287be11e27b29cc18dac2d8",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "fb7cbedae854b6df578962c7088dde4b4d0fec2acfc3e019ba5cf21de3a6aee8",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "e7fe1a77506aebc71c42027371d3ac4795f5bbf94bf153888eb8b6730050f080",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "6a87c46d10e65daf1de51e7893cb5838f8edfb37676496a848c989da99e9e32b",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "9e02fb86b5061741069f16f419b123085a47fd7bfa47b09505ec3921fa72672f",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d622305feea9f183265ab8ce550b0c1e11741a026ed5c3deb24f5d49f4b7502d",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "68f6b8542783dfeeeb5b06e70ce08ffefd75f3fa01876bd86a703f10e895df07",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "cbe1feba92e40ce6fbc8044dfda45028cf5293d2f310bf7f90c76f8a78712655",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "e1b40c548c1c0d5ff3cf09cd0e55e541666623f172e992589d10531d80c15fc7",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "1ea8aaac3f384ff9f47d811207f4708f00924520de9bb62417ca84682a318024",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "963e315dba2a02df4201bba7bc72a37ef1e49c2570989e56a25b597e831eba7e",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "5c02ba13c84a3935a2a3cb7c52167e11869d0ae5c0d5f13c3dfa4b328e7c79c0",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "6fa2d990059943937be369478640da20b3e960fd626f217c6c38ba21348560a9",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "9eba0cc3915ac7e83754a352a6985c3afe39cdabc551c5eeb9609377bae52757",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "7f9460a23d474b66c3b41d1793b353d6646bd35404739e0dd34dab1767ea1f34",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d0d516778fb0618769f1f2bf8f076661c6ff815124e34ffc58bec2cacd9063ec",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "02292b0887db3c979f796a57e0321efc800d59722771bc762e45a0052b8c5bdb",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "10b668847507e38270cf0501cf4185f634c8f01116662d894bcfe7b5207256f2",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "f6a16b6054bf7ac1e01efce48208425eeaf7deb95dae05a07bcffb0d1840a484",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "716c92116b36d29159cf3e324c399409acdc817c6fe7ae3587dc4e6e638f9111",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "948f395d3e8454456a3efdd5ad2652c1bc7c0ed17ff78b361617acdcdbc10ba4",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "98ba0006d11ae9b572a41300a1b5a9ef5a63d297ffd0987c1a6b40ca6da2be0f",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "9c39cb60a33d563d2958f49f1b6ee94582f2d7a846967f0dcec9b4d78e6d8483",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "f097bf1ee493e51003d196b7bca16546bbade52f3dbe436894b3356f6b2c50fb",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "cbc809ae94d7802f33e8a4eb3b323ed141067a6336fe96a71363784b6c7f27bf",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "bd9cbb0e18115c85d7aaceb5dfdcdb0507ad4dd8903daa9aecc69d63caa30fa6",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "8bfbee24c6e5261837151ef349506cf4822c3c1cb43b223bbe5cc18760a81cad",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d75127caa3409b590cfd289fe71be1518d9fc8ecacfd23766986b9515ff0013e",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "b8181bc6badbe124c07879421041e8f176b07b2bd63afc5cc21115782fb33b93",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "bb012f5118cce7cadeef104be967dd75f9acc5b14bf627dc911a5e167a54ead6",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "3d9285261122993897566e7430181e25e2fd65f1b1b173747dda97c55c38d4e4",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "d32ca0a5d468eab53ff057035c2869eacc0ab138197abc7f6faaef00c0ccf0d3",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "5551c1b53b0878eae3fedcfbd314c30f0d23b7fe32845dc2d9de4a8bd4904297",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "2faaf850def03ffa2dc6f85e00e65a5cf2be7383db6967a6ddbdec1a85153809",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "6a9cb41f4e7d95cd6bf7491e6ef96a09274b136a6beb56c9b1488fe58e2058c2",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "edd93d45ec97a96602b907a608a21208050886aa9aa7f367ea95ab2bc86b1495",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "dab5fc7fe5706d90b6ac29ed3f203add378c76b944d45159245cdfd17a53cbf5",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "97f6cc216d0557b45039598fd8b4919787b15173c166c997bf8c1d4b3052fb1a",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "f50b99b7468810b73a5a7dcf08bd17915c08966a2ecc1e07606304b1a44e8de3",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "e2b5061e55f18f0c8789973a2dedcebbe975f18d21721e854a3f3ba6db7f3588",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "2a148954227198c9b09c743020a46e97d83321fba27ad800a9103a2d60dc749c",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "aa215a188974e422ea3c2440a37608c4bef92dcdc39b6f01f2e1f28ff0fa3cf2",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "0a5a4878f4cc4453cc7a3ec58a642c871ebb7d208e87bdcc1e0cc372dd962abc",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "f7148b2288b0dc574d72b14df29030175832a64c8e50a9fae85daabc9e2cc3fa",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "f5e3efcb5a92f1b320feb9fbdd6c93fcceceaf38ed9768e793925b40128aa5cd",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "76cfa2899bf3de054e51e2dfeb07bd2b84e0a6f4b110cec9ab2799c7ceaa0fb4",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "042531823ff5ed0b0d77f4880da0b0e07400f246c3cdaa859e1994056037cfb4",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "c90e8bccfae00f7e628ca4d0c8661faceea3e4f534ec5dd400b7325ed8775d23",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "b0d7a6dbed426d40dbaa6c128d47df6637385a1378241662fd6e1483e66b5a8b",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "f652634815abf316fd1c0d687ef4a316239b77371572068bf6c4fb12cc550ae3",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "040bf36bdea2de0a4542cb5c539a534c68723275964aa5eb9c1a74b282375988",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "e8b5b2b681dad75337d16867767ee3c5977abe33ae04f7f080d2a3d4793301cb",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "3754e99f0786e2e13d70139bfcd5ce698be44e3392eb68e10e789e3ff0cdb59d",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "24c2f87496addcbe012f3c27206480db61019b6f632734f04239c0ffeb6a8499",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "9bbf06dad9ab5905e05471ce16d5222c89c2caa39f26267ac0747129885fbd44",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "1bcc7fa84de120a36755daf30a6f47e8c0d4bddc15036ed2a3447dfa7a1d3e88",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "f8ef49b4bbedf4164adaab192fbfb78c90599d29a4c5181f3ce59840d8490005",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "979e67a7ee97216402bbd77665a4b73a58f2128837deee671a462e83bd62afde",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "be63ab41d2c743f01b3a9b22d66d9101fe44d4fc2e04eba7eddeea19f87b338b",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "1f125f145e22f4758d3021bbb90b353b7053cb7e45930daf9d212f2591b00a85",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "fc1d6f42b05d00e6aeda82df286eb6e2578042f6caebe72144342466113bd81e",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "6d2cdf43ac02382a93d74bd97fd76ce63976b531546dc8dc41e3d4415d7ee1ea",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "b12fadf52943dfa54943fccb3b7893e796357686c10319e31ead3233444c0448",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "02e2459599b0f4ace96f8f6677b2a6a71d05e70332fc4df296575e76b369de57",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "6054b2225e75b6a4be2df29f6cea3b793bf2d82d8d98b36858627e5e8b47e27b",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "6e7106f3b190d8edb0760cd1f574e0f81d789cf5afad1987001829e764ae4b62",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "01ee7fc202708cfeb0c2bf930bf33a68ad086d4ce99a11e38d93ca698eb99805",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "9655cef01b024882124be02ef3455711811836ea35be799b09fd5f4e10eeccaf",
        )),
    });
    t.push(P256Point::Affine {
        x: BigUInt::from_be_bytes(&hex_lit(
            "77b20a912e6b23135066e911891524bc4efe3560e3e92350b52dec8f375f2b54",
        )),
        y: BigUInt::from_be_bytes(&hex_lit(
            "a3dc291825cea3f7f7b10bfcdd038a72df623da1e850e0f1caa801fcd6cc67ff",
        )),
    });
    t
}

fn hex_lit(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
