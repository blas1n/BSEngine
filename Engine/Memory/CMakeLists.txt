set_project ()

# Files
set(SRC_MANAGER
	MemoryManager.cpp
	MemoryManager.h
	MemoryManagerAccesser.cpp
	MemoryManagerAccesser.h
)

set(SRC_MEMORY
	ComponentMemory.h
	HeapMemory.cpp
	HeapMemory.h
	ManagerMemory.h
	OneFrameMemory.cpp
	OneFrameMemory.h
	ResourceMemory.h
)

set(SRC_ALLOCATOR
	ComponentAllocator.h
	HeapAllocator.h
	ManagerAllocator.h
	OneFrameAllocator.h
	ResourceAllocator.h
)

# Target
add_library (${PROJECT_ID} STATIC
	${SRC_MANAGER}
	${SRC_MEMORY}
	${SRC_ALLOCATOR}
)

# Dependencies
set_public_dependencies (GameBase)

# Filters
source_group ("Manager" FILES ${SRC_MANAGER})
source_group ("Memory" FILES ${SRC_MEMORY})
source_group ("Allocator" FILES ${SRC_ALLOCATOR})