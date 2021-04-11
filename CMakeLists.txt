cmake_minimum_required (VERSION 3.10)

project (BS_Engine
	VERSION 0.1.0
	LANGUAGES CXX
	DESCRIPTION "Game Engine made by blAs1N"
	HOMEPAGE_URL "https://github.com/blAs1N/BS_Engine"
)

if (NOT ${CMAKE_PROJECT_NAME} STREQUAL ${PROJECT_NAME})
	set (BUILD_GAME TRUE)
endif ()

set (CMAKE_CXX_STANDARD 17)
set (CMAKE_DIR "${PROJECT_SOURCE_DIR}/CMake")

include (CMake/Util.cmake)
include (CMake/ThirdParty.cmake)

set (CMAKE_RUNTIME_OUTPUT_DIRECTORY $<1:${CMAKE_SOURCE_DIR}/Binaries>)
set (CMAKE_LIBRARY_OUTPUT_DIRECTORY $<1:${CMAKE_SOURCE_DIR}/Binaries>)

set_property (GLOBAL PROPERTY USE_FOLDERS ON)

add_subdirectory (Source)

if (${PLATFORM_WINDOWS})
	install (DIRECTORY ${CMAKE_RUNTIME_OUTPUT_DIRECTORY} DESTINATION "." FILES_MATCHING PATTERN "*.dll")
else ()
	install (DIRECTORY ${CMAKE_RUNTIME_OUTPUT_DIRECTORY} DESTINATION "." FILES_MATCHING PATTERN "*.so")
endif ()

if (GTest_FOUND)
	file(GLOB_RECURSE TEST_FILES "Tests/*.cpp")
	target_link_libraries(Tests PRIVATE GTest::GTest)

	enable_testing()
	add_test(NAME Test COMMAND Tests)
endif ()