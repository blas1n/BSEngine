set_project ()

# Files
set (SRC_ALLOCATOR
	ComponentAllocator.h
	HeapAllocator.h
	ManagerAllocator.h
	OneFrameAllocator.h
	ResourceAllocator.h
)

set (SRC_MANAGER
	MemoryManager.cpp
	MemoryManager.h
	MemoryManagerAccesser.cpp
	MemoryManagerAccesser.h
)

set (SRC_MEMORY
	ComponentMemory.h
	HeapMemory.cpp
	HeapMemory.h
	ManagerMemory.h
	OneFrameMemory.cpp
	OneFrameMemory.h
	ResourceMemory.h
)

# Target
add_library (${PROJECT_ID} STATIC
	${SRC_ALLOCATOR}
	${SRC_MANAGER}
	${SRC_MEMORY}
)

# Dependencies
set_public_dependencies (GameBase Math)

# Filters
source_group ("Allocator" FILES ${SRC_ALLOCATOR})
source_group ("Manager" FILES ${SRC_MANAGER})
source_group ("Memory" FILES ${SRC_MEMORY})