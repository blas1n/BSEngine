#include "Matrix3x3.h"

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