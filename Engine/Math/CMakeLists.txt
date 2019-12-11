set_project ()

# Files
set (SRC_FUNCTION
	MathFunctions.cpp
	MathFunctions.h	
)

set (SRC_MATRIX
	Matrix3x3.cpp
	Matrix3x3.h
	Matrix4x4.cpp
	Matrix4x4.h
)

set (SRC_RANDOM
	Random.cpp
	Random.h
)

set (SRC_ROTATION
	Quaternion.cpp
	Quaternion.h
	Rotator.cpp
	Rotator.h
)

set (SRC_VECTOR
	Vector2.cpp
	Vector2.h
	Vector3.cpp
	Vector3.h
	Vector4.cpp
	Vector4.h
)

# Target
add_library (${PROJECT_ID} STATIC
	${SRC_FUNCTION}
	${SRC_MATRIX}
	${SRC_ROTATION}
	${SRC_VECTOR}
	${SRC_RANDOM}
)

# Dependencies
target_link_libraries (${PROJECT_ID} PUBLIC Eigen3::Eigen)

# Filters
source_group ("Function" FILES ${SRC_FUNCTION})
source_group ("Matrix" FILES ${SRC_MATRIX})
source_group ("Random" FILES ${SRC_RANDOM})
source_group ("Rotation" FILES ${SRC_ROTAION})
source_group ("Vector" FILES ${SRC_VECTOR})