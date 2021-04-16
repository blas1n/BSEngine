set (CMAKE_TOOLCHAIN_FILE ${CMAKE_CURRENT_SOURCE_DIR}/ThirdParty/vcpkg/scripts/buildsystems/vcpkg.cmake CACHE STRING "Vcpkg toolchain file")

find_package (SDL2 CONFIG REQUIRED)
find_package (GLEW REQUIRED)
find_package (fmt CONFIG REQUIRED)
find_package (spdlog CONFIG REQUIRED)
find_package (rapidJSON CONFIG REQUIRED)
find_package (utf8cpp CONFIG REQUIRED)

if (CMAKE_SYSTEM_NAME MATCHES "Linux")
	find_package (Threads REQUIRED)
endif ()

find_package(GTest)