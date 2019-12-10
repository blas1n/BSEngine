set_project ()

# Files
set(SRC_CONTAINER
	ArrayImpl.h
	Array.h
	Pair.h
	Queue.h
	Set.h
	SetImpl.h
	Stack.h
	Tuple.h
)

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
	Rotator.cpp
	Rotator.h
	Vector2.cpp
	Vector2.h
	Vector3.cpp
	Vector3.h
	Vector4.cpp
	Vector4.h
)

set(SRC_MISC
	Comparison.h
	Hash.h
	Macro.h
	Swap.h
	Type.h
)

set(SRC_STRING
	Character.h
	CString.h
)

set(SRC
	Core.h
)

# Package
if (CMAKE_SYSTEM_NAME MATCHES "Linux")
	find_package (Threads)
endif ()

find_package(Eigen3 REQUIRED)

# Target
add_library (${PROJECT_ID} STATIC
	${SRC_CONTAINER}
	${SRC_EXCEPTION}
	${SRC_MATH}
	${SRC_MISC}
	${SRC_STRING}
	${SRC}
)

# Dependencies
if (CMAKE_SYSTEM_NAME MATCHES "Linux")
	target_link_libraries (${PROJECT_ID} PUBLIC ${CMAKE_THREAD_LIBS_INIT})
endif ()

target_link_libraries(${PROJECT_ID} PUBLIC Eigen3::Eigen)

# Filters
source_group ("Container" FILES ${SRC_CONTAINER})
source_group ("Exception" FILES ${SRC_EXCEPTION})
source_group ("Math" FILES ${SRC_MATH})
source_group ("Misc" FILES ${SRC_MISC})
source_group ("String" FILES ${SRC_STRING})
source_group ("" FILES ${SRC})