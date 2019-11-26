set_project ()

# Files
set(SRC_EXCEPTION
	Exception.h
)

set(SRC_MATH
	MathFunctions.cpp
	MathFunctions.h
	Matrix3x3.cpp
	Matrix3x3.h
	Matrix4x4.cpp
	Matrix4x4.h
	Quaternion.cpp
	Quaternion.h
	Random.cpp
	Random.h
	Vector2.cpp
	Vector2.h
	Vector3.cpp
	Vector3.h
	Vector4.cpp
	Vector4.h
)

set(SRC_STRING
	Character.h
	CString.h
)

set(SRC
	Core.h
	Macro.h
	Type.h
)

# Target
add_library (${PROJECT_ID} STATIC
	${SRC_EXCEPTION}
	${SRC_MATH}
	${SRC_STRING}
	${SRC}
)

# Filters
source_group ("Container" FILES ${SRC_CONTAINER})
source_group ("Exception" FILES ${SRC_EXCEPTION})
source_group ("Math" FILES ${SRC_MATH})
source_group ("String" FILES ${SRC_STRING})
source_group ("" FILES ${SRC})