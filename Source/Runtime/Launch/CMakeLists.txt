# Source/Runtime/Launch

set (SRCS "Private/Launch.cpp")

if (${PLATFORM_WINDOWS})
	add_executable (${CMAKE_PROJECT_NAME} WIN32 "Private/LaunchWindows.cpp" ${SRCS})
else ()
	message (FATAL_ERROR "Unsupported OS!")
endif ()

target_link_libraries (${CMAKE_PROJECT_NAME} PRIVATE Engine)