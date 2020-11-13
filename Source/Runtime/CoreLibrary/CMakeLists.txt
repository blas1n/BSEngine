set_project ()

# Files
set (SRC_CORE
	Comparison.h
	Hash.h
	Macro.h
	Swap.h
	Templates.h
	Type.h
	Utility.h
)

set (SRC_EXCEPTION
	Exception.cpp
	Exception.h
)

set (SRC_STRING
	Character.h
	CString.h
)

set (SRC
	Core.h
)

# Target
add_library (${PROJECT_ID} STATIC
	${SRC_CORE}
	${SRC_EXCEPTION}
	${SRC_STRING}
	${SRC}
)

# Filters
source_group ("Core" FILES ${SRC_CORE})
source_group ("Exception" FILES ${SRC_EXCEPTION})
source_group ("String" FILES ${SRC_STRING})
source_group ("" FILES ${SRC})