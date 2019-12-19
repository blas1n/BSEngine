list (APPEND CMAKE_MODULE_PATH ${CMAKE_SOURCE_DIR}/CMake/Modules)
list (APPEND CMAKE_PREFIX_PATH ${CMAKE_SOURCE_DIR}/External)

if (CMAKE_SYSTEM_NAME MATCHES "Linux")
	find_package (Threads REQUIRED)
endif ()

find_package (Eigen3 REQUIRED)
