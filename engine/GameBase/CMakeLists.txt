# Files
set(SRC_System
	System.cpp
	System.h
)

set(SRC_Accesser
	AccesserGenerator.h
)

# Target
add_library (GameBase STATIC
	${SRC_System}
	${SRC_Accesser}
)

# Filters
source_group ("System" FILES ${SRC_System})
source_group ("Accesser" FILES ${SRC_Accesser})

# Dependencies
target_link_libraries (GameBase
	Library
)