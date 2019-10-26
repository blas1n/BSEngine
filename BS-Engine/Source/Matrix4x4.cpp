#include "Matrix4x4.h"

const Matrix4x4 Matrix4x4::Zero
{
	0.0f, 0.0f, 0.0f, 0.0f,
	0.0f, 0.0f, 0.0f, 0.0f,
	0.0f, 0.0f, 0.0f, 0.0f,
	0.0f, 0.0f, 0.0f, 0.0f
};

const Matrix4x4 Matrix4x4::One
{
	1.0f, 1.0f, 1.0f, 1.0f,
	1.0f, 1.0f, 1.0f, 1.0f,
	1.0f, 1.0f, 1.0f, 1.0f,
	1.0f, 1.0f, 1.0f, 1.0f
};

const Matrix4x4 Matrix4x4::Identity
{
	1.0f, 0.0f, 0.0f, 0.0f,
	0.0f, 1.0f, 0.0f, 0.0f,
	0.0f, 0.0f, 1.0f, 0.0f,
	0.0f, 0.0f, 0.0f, 1.0f
};

inline float GetDeterminant(const Matrix4x4& mat) noexcept
{
	return
		mat[0][0] * mat[1][1] * mat[2][2] * mat[3][3] -
		mat[0][0] * mat[1][1] * mat[2][3] * mat[3][2] -
		mat[0][0] * mat[1][2] * mat[2][1] * mat[3][3] +
		mat[0][0] * mat[1][2] * mat[2][3] * mat[3][1] +
		mat[0][0] * mat[1][3] * mat[2][1] * mat[3][2] -
		mat[0][0] * mat[1][3] * mat[2][2] * mat[3][1] -
		mat[0][1] * mat[1][0] * mat[2][2] * mat[3][3] +
		mat[0][1] * mat[1][0] * mat[2][3] * mat[3][2] +
		mat[0][1] * mat[1][2] * mat[2][1] * mat[3][3] -
		mat[0][1] * mat[1][2] * mat[2][3] * mat[3][0] -
		mat[0][1] * mat[1][3] * mat[2][0] * mat[3][2] +
		mat[0][1] * mat[1][3] * mat[2][2] * mat[3][0] +
		mat[0][2] * mat[1][0] * mat[2][1] * mat[3][3] -
		mat[0][2] * mat[1][0] * mat[2][3] * mat[3][1] -
		mat[0][2] * mat[1][1] * mat[2][0] * mat[3][3] +
		mat[0][2] * mat[1][1] * mat[2][3] * mat[3][0] +
		mat[0][2] * mat[1][3] * mat[2][0] * mat[3][1] -
		mat[0][2] * mat[1][3] * mat[2][1] * mat[3][0] -
		mat[0][3] * mat[1][0] * mat[2][1] * mat[3][2] +
		mat[0][3] * mat[1][0] * mat[2][2] * mat[3][1] +
		mat[0][3] * mat[1][1] * mat[2][0] * mat[3][2] -
		mat[0][3] * mat[1][1] * mat[2][2] * mat[3][0] -
		mat[0][3] * mat[1][2] * mat[2][0] * mat[3][1] +
		mat[0][3] * mat[1][2] * mat[2][1] * mat[3][0];
}

void Matrix4x4::Inverted() noexcept
{
	const Matrix4x4 ret
	{
		// 00
		rows[1][1] * rows[2][2] * rows[3][3] +
		rows[1][2] * rows[2][3] * rows[3][1] +
		rows[1][3] * rows[2][1] * rows[3][2] -
		rows[1][1] * rows[2][3] * rows[3][2] -
		rows[1][2] * rows[2][1] * rows[3][3] -
		rows[1][3] * rows[2][2] * rows[3][1],

		// 01
		rows[0][1] * rows[2][3] * rows[3][2] +
		rows[0][2] * rows[2][1] * rows[3][3] +
		rows[0][3] * rows[2][2] * rows[3][1] -
		rows[0][1] * rows[2][2] * rows[3][3] -
		rows[0][2] * rows[2][3] * rows[3][1] -
		rows[0][3] * rows[2][1] * rows[3][2],

		// 02
		rows[0][1] * rows[1][2] * rows[3][3] +
		rows[0][2] * rows[1][3] * rows[3][1] +
		rows[0][3] * rows[1][1] * rows[3][2] -
		rows[0][1] * rows[1][3] * rows[3][2] -
		rows[0][2] * rows[1][1] * rows[3][3] -
		rows[0][3] * rows[1][2] * rows[3][1],

		// 03
		rows[0][1] * rows[1][3] * rows[2][2] +
		rows[0][2] * rows[1][1] * rows[2][3] +
		rows[0][3] * rows[1][2] * rows[2][1] -
		rows[0][1] * rows[1][2] * rows[2][3] -
		rows[0][2] * rows[1][3] * rows[2][1] -
		rows[0][3] * rows[1][1] * rows[2][2],

		// 10
		rows[1][0] * rows[2][3] * rows[3][2] +
		rows[1][2] * rows[2][0] * rows[3][3] +
		rows[1][3] * rows[2][2] * rows[3][0] -
		rows[1][0] * rows[2][2] * rows[3][3] -
		rows[1][2] * rows[2][3] * rows[3][0] -
		rows[1][3] * rows[2][0] * rows[3][2],

		// 11
		rows[0][0] * rows[2][2] * rows[3][3] +
		rows[0][2] * rows[2][3] * rows[3][0] +
		rows[0][3] * rows[2][0] * rows[3][2] -
		rows[0][0] * rows[2][3] * rows[3][2] -
		rows[0][2] * rows[2][0] * rows[3][3] -
		rows[0][3] * rows[2][2] * rows[3][0],

		// 12
		rows[0][0] * rows[1][3] * rows[3][2] +
		rows[0][2] * rows[1][0] * rows[3][3] +
		rows[0][3] * rows[1][2] * rows[3][0] -
		rows[0][0] * rows[1][2] * rows[3][3] -
		rows[0][2] * rows[1][3] * rows[3][0] -
		rows[0][3] * rows[1][0] * rows[3][2],

		// 13
		rows[0][0] * rows[1][1] * rows[2][3] +
		rows[0][1] * rows[1][3] * rows[2][0] +
		rows[0][3] * rows[1][0] * rows[2][1] -
		rows[0][0] * rows[1][3] * rows[2][2] -
		rows[0][2] * rows[1][0] * rows[2][3] -
		rows[0][3] * rows[1][2] * rows[2][0],

		// 20
		rows[1][0] * rows[2][1] * rows[3][3] +
		rows[1][1] * rows[2][3] * rows[3][0] +
		rows[1][3] * rows[2][0] * rows[3][1] -
		rows[1][0] * rows[2][3] * rows[3][1] -
		rows[1][1] * rows[2][0] * rows[3][3] -
		rows[1][3] * rows[2][1] * rows[3][0],

		// 21
		rows[0][0] * rows[2][3] * rows[3][1] +
		rows[0][1] * rows[2][0] * rows[3][3] +
		rows[0][3] * rows[2][1] * rows[3][0] -
		rows[0][0] * rows[2][1] * rows[3][3] -
		rows[0][1] * rows[2][3] * rows[3][0] -
		rows[0][3] * rows[2][0] * rows[3][1],

		// 22
		rows[0][0] * rows[1][1] * rows[3][3] +
		rows[0][1] * rows[1][3] * rows[3][0] +
		rows[0][3] * rows[1][0] * rows[3][1] -
		rows[0][0] * rows[1][3] * rows[3][1] -
		rows[0][1] * rows[1][0] * rows[3][3] -
		rows[0][3] * rows[1][1] * rows[3][0],

		// 23
		rows[0][0] * rows[1][1] * rows[2][3] +
		rows[0][1] * rows[1][3] * rows[2][0] +
		rows[0][3] * rows[1][0] * rows[2][1] -
		rows[0][0] * rows[1][3] * rows[2][1] -
		rows[0][1] * rows[1][0] * rows[2][3] -
		rows[0][3] * rows[1][1] * rows[2][0],

		// 30
		rows[1][0] * rows[2][2] * rows[3][1] +
		rows[1][1] * rows[2][0] * rows[3][2] +
		rows[1][2] * rows[2][1] * rows[3][0] -
		rows[1][0] * rows[2][1] * rows[3][2] -
		rows[1][1] * rows[2][2] * rows[3][0] -
		rows[1][2] * rows[2][0] * rows[3][1],

		// 31
		rows[0][0] * rows[2][1] * rows[3][2] +
		rows[0][1] * rows[2][2] * rows[3][0] +
		rows[0][2] * rows[2][0] * rows[3][1] -
		rows[0][0] * rows[2][2] * rows[3][1] -
		rows[0][1] * rows[2][0] * rows[3][2] -
		rows[0][2] * rows[2][1] * rows[3][0],

		// 32
		rows[0][0] * rows[1][2] * rows[3][1] +
		rows[0][1] * rows[1][0] * rows[3][2] +
		rows[0][2] * rows[1][1] * rows[3][0] -
		rows[0][0] * rows[1][1] * rows[3][2] -
		rows[0][1] * rows[1][2] * rows[3][0] -
		rows[0][2] * rows[1][0] * rows[3][1],

		// 33
		rows[0][0] * rows[1][1] * rows[2][2] +
		rows[0][1] * rows[1][2] * rows[2][0] +
		rows[0][2] * rows[1][0] * rows[2][1] -
		rows[0][0] * rows[1][2] * rows[2][1] -
		rows[0][1] * rows[1][0] * rows[2][2] -
		rows[0][2] * rows[1][1] * rows[2][0],
	};

	*this = ret / GetDeterminant(*this);
}