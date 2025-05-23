#! /bin/sh
# EndBASIC
# Copyright 2021 Julio Merino
#
# Licensed under the Apache License, Version 2.0 (the "License"); you may not
# use this file except in compliance with the License.  You may obtain a copy
# of the License at:
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
# WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.  See the
# License for the specific language governing permissions and limitations
# under the License.

set -eux

readonly PROGNAME="${0##*/}"

err() {
    echo "${PROGNAME}: ${*}" 1>&2
    exit 1
}

sanity_check() {
    local bin="${1}"; shift

    local ret=0
    echo "END 123" | "${bin}" || ret="${?}"
    [ "${ret}" -eq 123 ] || err "Packaged endbasic doesn't seem to work"
}

main() {
    [ "${#}" -eq 1 ] || err "Must provide a release configuration"
    local name="${1}"; shift

    local version=
    case "${GITHUB_REF-}" in
        refs/heads/endbasic-*|refs/tags/endbasic-*)
            version="${GITHUB_REF#*-}"

            local cargo_version="$(grep ^version core/Cargo.toml | head -n 1 | cut -d '"' -f 2)"
            [ "${version}" = "${cargo_version}" ] \
                || err "Cargo.toml version doesn't match branch name"
            ;;

        *)
            version="$(git rev-parse --short ${GITHUB_SHA})"
            ;;
    esac
    [ -n "${version}" ] || err "Cannot determine version number"

    local notices="$(echo NOTICE */NOTICE)"

    local distname="endbasic-${version}-${name}"
    local outdir="endbasic-${name}"
    mkdir -p "${distname}" "${outdir}"

    cp LICENSE NEWS.md README.md "${distname}"

    for f in ${notices}; do
        echo "# ${f}"
        echo
        cat "${f}"
        echo
    done >"${distname}/NOTICE"

    local target=
    case "${name}" in
        linux-armv6-rpi)
            # Add the RPI toolchain in order to use the Rpi toolchain
            # See https://github.com/mdirkse/rust_armv6
            git  clone --depth=1 https://github.com/raspberrypi/tools.git "rpi_tools"
            # Remove most of the repo we just downloaded as we only need a small amount
            rm -fr "rpi_tools/.git" \
                   "rpi_tools/arm-bcm2708/arm-bcm2708-linux-gnueabi" \
                   "rpi_tools/arm-bcm2708/arm-bcm2708hardfp-linux-gnueabi" \
                   "rpi_tools/arm-bcm2708/gcc-linaro-arm-linux-gnueabihf-raspbian" \
                   "rpi_tools/arm-bcm2708/gcc-linaro-arm-linux-gnueabihf-raspbian-x64"
            # TODO(jmmv): Should figure out how to cross-compile with the native TLS library
            # instead of doing this hack.
            sed -i s,native-tls,rustls-tls,g client/Cargo.toml
            # TODO(jmmv): Should enable --features=sdl but need to figure out how to cross-build
            # for it.
            ( cd cli && cargo build --target arm-unknown-linux-gnueabihf   --config target.arm-unknown-linux-gnueabihf.linker=\"$(realpath ../rpi_tools/arm-bcm2708/arm-rpi-4.9.3-linux-gnueabihf/bin/arm-linux-gnueabihf-gcc)\" --release --features=rpi )
            cp ./target/arm-unknown-linux-gnueabihf/release/endbasic "${distname}"
            ;;
            
        linux-armv7-rpi)
            [ ! -f .cargo/config ] || err "Won't override existing .cargo/config"
            cp .cargo/config.rpi .cargo/config
            # TODO(jmmv): Should figure out how to cross-compile with the native TLS library
            # instead of doing this hack.
            sed -i s,native-tls,rustls-tls,g client/Cargo.toml
            # TODO(jmmv): Should enable --features=sdl but need to figure out how to cross-build
            # for it.
            ( cd cli && cargo build --release --features=rpi )
            rm -f .cargo/config

            cp ./target/armv7-unknown-linux-gnueabihf/release/endbasic "${distname}"
            ;;

        macos*)
            brew install sdl2 sdl2_ttf

            local brew="$(brew --prefix)"
            (
                cd cli
                export LIBRARY_PATH="${brew}/lib"
                cargo build --release --features=sdl
            )

            cp ./target/release/endbasic "${distname}/endbasic.bin"
            cp .github/workflows/macos-launcher.sh "${distname}/endbasic"

            # Bundle the necessary shared libraries as provided by Homebrew.
            cp "${brew}"/Cellar/sdl2/*/lib/libSDL2-*.dylib "${distname}"
            cp "${brew}"/Cellar/sdl2/*/LICENSE.txt "${distname}/LICENSE.sdl2"
            cp "${brew}"/Cellar/sdl2_ttf/*/lib/libSDL2_ttf-*.dylib "${distname}"
            cp "${brew}"/Cellar/sdl2_ttf/*/LICENSE.txt "${distname}/LICENSE.sdl2_ttf"
            cp "${brew}"/Cellar/freetype/*/lib/libfreetype.*.dylib "${distname}"
            cp "${brew}"/Cellar/freetype/*/LICENSE.TXT "${distname}/LICENSE.freetype"
            cp "${brew}"/Cellar/libpng/*/lib/libpng16.*.dylib "${distname}"
            cp "${brew}"/Cellar/libpng/*/LICENSE "${distname}/LICENSE.libpng"

            brew uninstall --ignore-dependencies sdl2 sdl2_ttf freetype libpng
            sanity_check "${distname}/endbasic"
            ;;

        windows*)
            ( cd cli && LIB="$(pwd)/libs" cargo build --release --features=sdl )

            cp ./target/release/endbasic.exe "${distname}"
            cp dlls/* "${distname}"

            sanity_check "${distname}/endbasic.exe"
            ;;

        *)
            ( cd cli && cargo build --release --features=sdl )

            cp ./target/release/endbasic "${distname}"

            sanity_check "${distname}/endbasic"
            ;;
    esac
    zip -r "${outdir}/${distname}.zip" "${distname}"
    rm -rf "${distname}"
}

main "${@}"
