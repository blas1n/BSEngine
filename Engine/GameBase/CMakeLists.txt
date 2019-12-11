set_project ()

# Files
set(SRC_System
	System.cpp
	System.h
)

set(SRC_Accesser
	AccesserGenerator.h
)

# Target
add_library (${PROJECT_ID} STATIC
	${SRC_System}
	${SRC_Accesser}
)

# Dependencies
set_public_dependencies (Library)
set_private_dependencies (Memory)

# Filters
source_group ("System" FILES ${SRC_System})
source_group ("Accesser" FILES ${SRC_Accesser})