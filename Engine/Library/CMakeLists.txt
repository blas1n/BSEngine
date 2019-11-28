set_project ()

# Files
set(SRC_EXCEPTION
	Exception.cpp
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

# Package
if (CMAKE_SYSTEM_NAME MATCHES "Linux")
	find_package (Threads)
endif ()

# Target
add_library (${PROJECT_ID} STATIC
	${SRC_EXCEPTION}
	${SRC_MATH}
	${SRC_STRING}
	${SRC}
)

# Dependencies
if (CMAKE_SYSTEM_NAME MATCHES "Linux")
	target_link_libraries (${PROJECT_ID} ${CMAKE_THREAD_LIBS_INIT})
endif ()

target_include_directories(${PROJECT_ID} PUBLIC ${CMAKE_SOURCE_DIR}/External/Eigen3)

# Filters
source_group ("Container" FILES ${SRC_CONTAINER})
source_group ("Exception" FILES ${SRC_EXCEPTION})
source_group ("Math" FILES ${SRC_MATH})
source_group ("String" FILES ${SRC_STRING})
source_group ("" FILES ${SRC})