#!/usr/bin/env bash
# Shared macOS native-build environment for GGML-based dependencies.
# whisper.cpp / llama.cpp use C++ std::filesystem APIs that require macOS 10.15+.
# Keep Cargo's deployment target and the CMake sub-build target aligned.
export MACOSX_DEPLOYMENT_TARGET="10.15"
export CMAKE_OSX_DEPLOYMENT_TARGET="10.15"
case " ${CMAKE_ARGS:-} " in
  *" -DCMAKE_OSX_DEPLOYMENT_TARGET="*) ;;
  *) export CMAKE_ARGS="-DCMAKE_OSX_DEPLOYMENT_TARGET=${MACOSX_DEPLOYMENT_TARGET}${CMAKE_ARGS:+ ${CMAKE_ARGS}}" ;;
esac
