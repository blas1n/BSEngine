list (APPEND CMAKE_MODULE_PATH ${CMAKE_SOURCE_DIR}/CMake/Modules)
set (CMAKE_INSTALL_PREFIX ${CMAKE_SOURCE_DIR}/External)

if (CMAKE_SYSTEM_NAME MATCHES "Linux")
	find_package (Threads REQUIRED)
endif ()

find_package (Eigen3 REQUIRED)
