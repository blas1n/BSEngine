cmake_minimum_required (VERSION 3.8)

project (BS_Engine VERSION 0.0.1 LANGUAGES CXX)

list (APPEND CMAKE_MODULE_PATH ${CMAKE_SOURCE_DIR}/CMake)
list (APPEND CMAKE_MODULE_PATH ${CMAKE_SOURCE_DIR}/CMake/Module)
list (APPEND CMAKE_PREFIX_PATH ${CMAKE_SOURCE_DIR}/External)

foreach (PATH ${CMAKE_MODULE_PATH})
    message (${PATH})
endforeach()

include (Platform)
include (Package)
include (Util)

set (CMAKE_CXX_STANDARD 17)
set (CMAKE_CXX_STANDARD_REQUIRED ON)
set (CMAKE_CXX_EXTENSIONS OFF)

set_property (GLOBAL PROPERTY USE_FOLDERS ON)

add_subdirectory (Engine)
