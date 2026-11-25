EAPI=8

inherit savedconfig toolchain-funcs

DESCRIPTION="Dynamic window manager for X11 (dwm-forge patched build)"
HOMEPAGE="https://github.com/isodiff/dwm-forge"
SRC_URI="dwm-patched-${PV}.tar.gz"
RESTRICT="fetch"

LICENSE="MIT"
SLOT="0"
KEYWORDS="~amd64 amd64"
IUSE="+savedconfig xinerama"

RDEPEND="
	media-libs/fontconfig
	x11-libs/libX11
	x11-libs/libXft
	xinerama? ( x11-libs/libXinerama )
"
DEPEND="${RDEPEND}"

S="${WORKDIR}"

src_unpack() {
	mkdir -p "${S}" || die
	tar -xzf "${DISTDIR}/${A}" --strip-components=1 -C "${S}" || die
}

src_prepare() {
	default
	restore_config config.h
}

src_compile() {
	emake CC="$(tc-getCC)" \
		XINERAMAFLAGS="$(usex xinerama '-DXINERAMA' '')" \
		XINERAMALIBS="$(usex xinerama '-lXinerama' '')" \
		FREETYPEINC="/usr/include/freetype2"
}

src_install() {
	dobin dwm
	doman dwm.1
	save_config config.h
}
