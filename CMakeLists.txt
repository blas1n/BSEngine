cmake_minimum_required (VERSION 3.8)

project (BS_Engine VERSION 0.0.1 LANGUAGES CXX)

set (CMAKE_MODULE_PATH ${CMAKE_MODULE_PATH} ${CMAKE_SOURCE_DIR}/Cmake)
set (CMAKE_PREFIX_PATH ${CMAKE_PREFIX_PATH} ${CMAKE_SOURCE_DIR}/External)

include (Platform)
include (Util)

set (CMAKE_CXX_STANDARD 17)
set (CMAKE_CXX_STANDARD_REQUIRED ON)
set (CMAKE_CXX_EXTENSIONS OFF)

set_property (GLOBAL PROPERTY USE_FOLDERS ON)

add_subdirectory (Engine)