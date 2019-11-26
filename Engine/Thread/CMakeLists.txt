set_project ()

# Files
set(SRC_MANAGER
	ThreadManager.cpp
	ThreadManager.h
	ThreadManagerAccesser.cpp
	ThreadManagerAccesser.h
)

# Target
add_library (${PROJECT_ID} STATIC
	${SRC_MANAGER}
)

# Filters
source_group ("Manager" FILES ${SRC_MANAGER})

# Dependencies
set_public_dependencies (GameBase)

if (CMAKE_SYSTEM_NAME MATCHES "Linux")
target_link_libraries (${PROJECT_ID} PUBLIC ${CMAKE_THREAD_LIBS_INIT})
endif ()