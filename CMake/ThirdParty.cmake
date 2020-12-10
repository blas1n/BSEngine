if (CMAKE_SYSTEM_NAME MATCHES "Linux")
	find_package (Threads REQUIRED)
endif ()

find_package(SDL2 REQUIRED)
find_package(GLEW REQUIRED)
find_package(spdlog REQUIRED)
find_package(fmt REQUIRED)
find_package(rapidJSON REQUIRED)