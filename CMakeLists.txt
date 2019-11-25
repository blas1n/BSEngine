cmake_minimum_required (VERSION 3.8)

project (BS_Engine VERSION 0.0.1 LANGUAGES CXX)

include (Cmake/Platform.cmake)

set (CMAKE_CXX_STANDARD 17)
set (CMAKE_CXX_STANDARD_REQUIRED ON)
set (CMAKE_CXX_EXTENSIONS OFF)

set_property (GLOBAL PROPERTY USE_FOLDERS ON)

add_subdirectory (Engine)