# Cross-compiles the vendored libaom and libavif (meal photo AVIF encoding) for arm64 release
# images. cmake-rs reads it from CMAKE_TOOLCHAIN_FILE_aarch64_unknown_linux_gnu (see Dockerfile).
#
# libaom-sys passes -DAOM_TARGET_CPU=aarch64 when cross-compiling, a name aom 3.11 does not know,
# so aom would silently build plain C without NEON. Force aom's own name for the architecture;
# the rest mirrors aom's build/cmake/toolchains/arm64-linux-gcc.cmake.
set(CMAKE_SYSTEM_NAME Linux)
set(CMAKE_SYSTEM_PROCESSOR aarch64)
set(CMAKE_C_COMPILER aarch64-linux-gnu-gcc)
set(CMAKE_CXX_COMPILER aarch64-linux-gnu-g++)
set(CMAKE_ASM_COMPILER aarch64-linux-gnu-as)
set(CMAKE_C_FLAGS_INIT "-march=armv8-a")
set(CMAKE_CXX_FLAGS_INIT "-march=armv8-a")
set(AOM_AS_FLAGS "-march=armv8-a")
set(AOM_NEON_INTRIN_FLAG "")
set(AOM_TARGET_CPU "arm64" CACHE STRING "aom target CPU" FORCE)
