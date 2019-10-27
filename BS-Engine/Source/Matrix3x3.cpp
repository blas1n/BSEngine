#include "Matrix3x3.h"
#include "MathFunctions.h"

const Matrix3x3 Matrix3x3::Zero
{
	0.0f, 0.0f, 0.0f,
	0.0f, 0.0f, 0.0f,
	0.0f, 0.0f, 0.0f
};

const Matrix3x3 Matrix3x3::One
{
	1.0f, 1.0f, 1.0f,
	1.0f, 1.0f, 1.0f,
	1.0f, 1.0f, 1.0f
};

const Matrix3x3 Matrix3x3::Identity
{
	1.0f, 0.0f, 0.0f,
	0.0f, 1.0f, 0.0f,
	0.0f, 0.0f, 1.0f
};

Matrix3x3 operator*(const Matrix3x3& lhs, const Matrix3x3& rhs) noexcept
{
	auto transRhs = Matrix3x3::Transpose(rhs);
	return Matrix3x3
	{
		Vector3::Dot(lhs[0], transRhs[0]),
		Vector3::Dot(lhs[0], transRhs[1]),
		Vector3::Dot(lhs[0], transRhs[2]),
		Vector3::Dot(lhs[1], transRhs[0]),
		Vector3::Dot(lhs[1], transRhs[1]),
		Vector3::Dot(lhs[1], transRhs[2]),
		Vector3::Dot(lhs[2], transRhs[0]),
		Vector3::Dot(lhs[2], transRhs[1]),
		Vector3::Dot(lhs[2], transRhs[2]),
	};
}

inline Matrix3x3 Matrix3x3::FromRotation(float theta) noexcept
{
	return Matrix3x3
	{
		Math::Cos(theta), Math::Sin(theta), 0.0f,
		-Math::Sin(theta), Math::Cos(theta), 0.0f,
		0.0f, 0.0f, 1.0f
	};
}

inline float GetDeterminant(const Matrix3x3& mat) noexcept
{
	return
		mat[0][0] * mat[1][1] * mat[2][2] +
		mat[0][1] * mat[1][2] * mat[2][0] +
		mat[0][2] * mat[1][0] * mat[2][1] -
		mat[0][2] * mat[1][1] * mat[2][0] -
		mat[0][1] * mat[1][0] * mat[2][2] -
		mat[0][0] * mat[1][2] * mat[2][1];
}

void Matrix3x3::Inverted() noexcept
{
	const Matrix3x3 ret
	{
		rows[1][1] * rows[2][2] - rows[1][2] * rows[2][1],
		rows[1][2] * rows[2][0] - rows[1][0] * rows[2][2],
		rows[1][0] * rows[2][1] - rows[1][1] * rows[2][0],
		rows[0][2] * rows[2][1] - rows[0][1] * rows[2][2],
		rows[0][0] * rows[2][2] - rows[0][2] * rows[2][0],
		rows[0][1] * rows[2][0] - rows[0][0] * rows[2][1],
		rows[0][1] * rows[1][2] - rows[0][2] * rows[1][1],
		rows[0][2] * rows[1][0] - rows[0][2] * rows[1][2],
		rows[0][0] * rows[1][1] - rows[0][1] * rows[1][0]
	};

	*this = ret / GetDeterminant(*this);
}