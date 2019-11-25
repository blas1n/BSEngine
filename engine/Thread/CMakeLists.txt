# Files
set(SRC_MANAGER
	ThreadManager.cpp
	ThreadManager.h
	ThreadManagerAccesser.cpp
	ThreadManagerAccesser.h
)

# Target
add_library (Thread STATIC
	${SRC_MANAGER}
)

# Filters
source_group ("Manager" FILES ${SRC_MANAGER})

# Dependencies
target_link_libraries (Thread
	GameBase
	Library
)

# Package
if (CMAKE_SYSTEM_NAME MATCHES "Linux")
find_package (Threads)
endif ()

target_link_libraries (Thread ${CMAKE_THREAD_LIBS_INIT})