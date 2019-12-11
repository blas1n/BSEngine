set_project ()

# Files
set (SRC
	ArrayImpl.h
	Array.h
	Map.h
	Pair.h
	Queue.h
	Set.h
	SetImpl.h
	Stack.h
	Tuple.h
)

# Target
add_library (${PROJECT_ID} STATIC
	${SRC}
)

# Dependencies
set_public_dependencies (Memory)

# Filters
source_group ("" FILES ${SRC})