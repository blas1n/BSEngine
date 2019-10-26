#pragma once

#include "Core.h"
#include "Vector2.h"
#include "Vector3.h"
#include "Vector4.h"
#include <initializer_list>
#include <memory>

template <uint8 Row, uint8 Column>
class BS_API Matrix
{
public:
	constexpr Matrix() noexcept;
	Matrix(std::initializer_list<float> elems) noexcept;
	Matrix(float elems[Row][Column]) noexcept;

	Matrix(const Matrix& other) noexcept = default;
	Matrix(Matrix&& other) noexcept = default;

	Matrix& operator=(const Matrix& other) noexcept = default;
	Matrix& operator=(Matrix&& other) noexcept = default;

	~Matrix() noexcept = default;

	static const Matrix& GetZero() noexcept;
	static const Matrix& GetOne() noexcept;
	static const Matrix& GetIdentity() noexcept;

	bool operator==(const Matrix& other) const noexcept;
	bool operator!=(const Matrix& other) const noexcept;

	Matrix& operator+=(const Matrix& other) noexcept;
	Matrix& operator-=(const Matrix& other) noexcept;

	Matrix& operator*=(float scaler) noexcept;
	Matrix& operator/=(float scaler) noexcept;

	Matrix& operator*=(const Matrix<Column, Column>& other) noexcept;

	template <uint8 Row, uint8 Column>
	friend Matrix<Row, Column> operator+(const Matrix<Row, Column>& lhs, const Matrix<Row, Column>& rhs) noexcept;
	
	template <uint8 Row, uint8 Column>
	friend Matrix<Row, Column> operator-(const Matrix<Row, Column>& lhs, const Matrix<Row, Column>& rhs) noexcept;

	template <uint8 Row, uint8 Column>
	friend Matrix<Row, Column> operator*(const Matrix<Row, Column>& mat, float scaler) noexcept;
	
	template <uint8 Row, uint8 Column>
	friend Matrix<Row, Column> operator*(float scaler, const Matrix<Row, Column>& mat) noexcept;
	
	template <uint8 Row, uint8 Column>
	friend Matrix<Row, Column> operator/(const Matrix<Row, Column>& mat, float scaler) noexcept;

	template<uint8 M, uint8 N, uint8 P>
	friend Matrix<M, P> operator*(const Matrix<M, N>& lhs, const Matrix<N, P>& rhs) noexcept;

	Matrix operator+() const noexcept;
	Matrix operator-() const noexcept;

	float* operator[](uint8 row) const noexcept;

	static Matrix<Column, Row> Transpose(const Matrix<Row, Column>& mat) noexcept;

	static Matrix<Row, Column> Invert(const Matrix<Row, Column>& mat) noexcept;
	void Inverted() noexcept;

	/// @todo Return custom string class
	const char* ToString() const noexcept;

private:
	float mat[Row][Column];

	/// @warning Do not use it as an operator for the underlying API.
	explicit operator const float* () const noexcept
	{
		return mat;
	}
};

template <uint8 Row, uint8 Column>
constexpr Matrix<Row, Column>::Matrix() noexcept
	: mat() {}

template <uint8 Row, uint8 Column>
Matrix<Row, Column>::Matrix(std::initializer_list<float> elems) noexcept
	: mat()
{
	auto iter = elems.begin();

	for (uint8 row = 0; row < Row; ++row)
		for (uint8 column = 0; column < Column; ++column)
			mat[row][column] = *(iter++);
}

template <uint8 Row, uint8 Column>
Matrix<Row, Column>::Matrix(float elems[Row][Column]) noexcept
	: mat()
{
	std::memcpy(mat, elems, Row * Column);
}

template <uint8 Row, uint8 Column>
const Matrix<Row, Column>& Matrix<Row, Column>::GetZero() noexcept
{
	static Matrix<Row, Column> zero;
	return zero;
}

template <uint8 Row, uint8 Column>
const Matrix<Row, Column>& Matrix<Row, Column>::GetOne() noexcept
{
	static Matrix<Row, Column> one;
	static bool isInit = false;

	if (!isInit)
	{
		for (uint8 row = 0; row < Row; ++row)
			for (uint8 column = 0; column < Column; ++column)
				one[row][column] = 1.0f;
		isInit = true;
	}

	return one;
}

template <uint8 Row, uint8 Column>
const Matrix<Row, Column>& Matrix<Row, Column>::GetIdentity() noexcept
{
	static Matrix<Row, Column> identity;
	static bool isInit = false;

	if (!isInit)
	{
		for (uint8 row = 0; row < Row; ++row)
			for (uint8 column = 0; column < Column; ++column)
				identity[row][column] = row == column ? 1.0f : 0.0f;
		isInit = true;
	}

	return identity;
}

template <uint8 Row, uint8 Column>
bool Matrix<Row, Column>::operator==(const Matrix& other) const noexcept
{
	for (uint8 row = 0; row < Row; ++row)
		for (uint8 column = 0; column < Column; ++column)
			if (mat[row][column] != other[row][column])
				return false;

	return true;
}

template <uint8 Row, uint8 Column>
bool Matrix<Row, Column>::operator!=(const Matrix& other) const noexcept
{
	return !(*this == other);
}

template <uint8 Row, uint8 Column>
Matrix<Row, Column>& Matrix<Row, Column>::operator+=(const Matrix& other) noexcept
{
	for (uint8 row = 0; row < Row; ++row)
		for (uint8 column = 0; column < Column; ++column)
			mat[row][column] += other[row][column];

	return *this;
}

template <uint8 Row, uint8 Column>
Matrix<Row, Column>& Matrix<Row, Column>::operator-=(const Matrix& other) noexcept
{
	for (uint8 row = 0; row < Row; ++row)
		for (uint8 column = 0; column < Column; ++column)
			mat[row][column] -= other[row][column];

	return *this;
}

template <uint8 Row, uint8 Column>
Matrix<Row, Column>& Matrix<Row, Column>::operator*=(const Matrix<Column, Column>& other) noexcept
{
	return (*this = *this * other);
}

template <uint8 Row, uint8 Column>
Matrix<Row, Column>& Matrix<Row, Column>::operator*=(float scaler) noexcept
{
	for (uint8 row = 0; row < Row; ++row)
		for (uint8 column = 0; column < Column; ++column)
			mat[row][column] *= scaler;

	return *this;
}

template <uint8 Row, uint8 Column>
Matrix<Row, Column>& Matrix<Row, Column>::operator/=(float scaler) noexcept
{
	for (uint8 row = 0; row < Row; ++row)
		for (uint8 column = 0; column < Column; ++column)
			mat[row][column] /= scaler;

	return *this;
}

template <uint8 Row, uint8 Column>
Matrix<Row, Column> operator+(const Matrix<Row, Column>& lhs, const Matrix<Row, Column>& rhs) noexcept
{
	Matrix<Row, Column> ret;

	for (uint8 row = 0; row < Row; ++row)
		for (uint8 column = 0; column < Column; ++column)
			ret[row][column] = lhs[row][column] + rhs[row][column];

	return ret;
}

template <uint8 Row, uint8 Column>
Matrix<Row, Column> operator-(const Matrix<Row, Column>& lhs, const Matrix<Row, Column>& rhs) noexcept
{
	Matrix<Row, Column> ret;

	for (uint8 row = 0; row < Row; ++row)
		for (uint8 column = 0; column < Column; ++column)
			ret[row][column] = lhs[row][column] - rhs[row][column];

	return ret;
}

template <uint8 Row, uint8 Column>
Matrix<Row, Column> operator*(const Matrix<Row, Column>& mat, const float scaler) noexcept
{
	Matrix<Row, Column> ret;

	for (uint8 row = 0; row < Row; ++row)
		for (uint8 column = 0; column < Column; ++column)
			ret[row][column] = mat[row][column] * scaler;

	return ret;
}

template <uint8 Row, uint8 Column>
Matrix<Row, Column> operator*(const float scaler, const Matrix<Row, Column>& mat) noexcept
{
	Matrix<Row, Column> ret;

	for (uint8 row = 0; row < Row; ++row)
		for (uint8 column = 0; column < Column; ++column)
			ret[row][column] = mat[row][column] * scaler;

	return ret;
}

template <uint8 Row, uint8 Column>
Matrix<Row, Column> operator/(const Matrix<Row, Column>& mat, float scaler) noexcept
{
	Matrix<Row, Column> ret;

	for (uint8 row = 0; row < Row; ++row)
		for (uint8 column = 0; column < Column; ++column)
			ret[row][column] = mat[row][column] / scaler;

	return ret;
}

template<uint8 M, uint8 N, uint8 P>
Matrix<M, P> operator*(const Matrix<M, N>& lhs, const Matrix<N, P>& rhs) noexcept
{
	constexpr static auto Dot = [](auto* lhs, auto* rhs)
	{
		auto ret = 0.0f;
		for (auto i = 0; i < N; ++i)
			ret += lhs[i] * rhs[i];
		return ret;
	};

	Matrix<M, P> ret;
	const auto transRhs = rhs.Transpose();

	for (uint8 i = 0; i < M; ++i)
	{
		const auto v1 = lhs[i];

		for (uint8 j = 0; j < P; ++j)
		{
			const auto v2 = rhs[j];
			ret[i][j] = Dot(v1, v2);
		}
	}

	return ret;
}

template <uint8 Row, uint8 Column>
Matrix<Row, Column> Matrix<Row, Column>::operator+() const noexcept
{
	return *this;
}

template <uint8 Row, uint8 Column>
Matrix<Row, Column> Matrix<Row, Column>::operator-() const noexcept
{
	return *this * -1.0f;
}

template <uint8 Row, uint8 Column>
float* Matrix<Row, Column>::operator[](uint8 row) const noexcept
{
	return const_cast<float*>(mat[row]);
}

template <uint8 Row, uint8 Column>
Matrix<Column, Row> Matrix<Row, Column>::Transpose(const Matrix<Row, Column>& mat) noexcept
{
	Matrix<Column, Row> ret;

	for (uint8 row = 0; row < Row; ++row)
		for (uint8 column = 0; column < Column; ++column)
			ret[column][row] = mat[row][column];

	return ret;
}

inline float Det(const Matrix<1, 1>& m)
{
	return m[0][0];
}

inline float Det(const Matrix<2, 2>& m)
{
	return m[0][0] * m[1][1] - m[0][1] * m[1][0];
}

inline float Det(const Matrix<3, 3>& m)
{
	return m[0][0] * m[1][1] * m[2][2] + m[0][1] * m[1][2] * m[2][0] + m[0][2] * m[1][0] * m[2][1]
		- m[0][2] * m[1][1] * m[2][0] - m[0][1] * m[1][0] * m[2][2] - m[0][0] * m[1][2] * m[2][1];
}

template <uint8 Dim>
float Det(const Matrix<Dim, Dim>& mat)
{
	float sign = 1.0f;
	float ret = 0.0f;

	for (uint8 i = 0; i < Dim; ++i)
	{
		Matrix<Dim - 1, Dim - 1> sub_matrix;

		for (uint8 src_col = 1; src_col < Dim; ++src_col)
		{
			const uint8 dst_col = src_col - 1u;

			for (uint8 src_row = 0, dst_row = 0; src_row < Dim; ++src_row)
			{
				if (src_row != i)
				{
					sub_matrix[dst_row][dst_col] = mat[src_row][src_col];
					++dst_row;
				}
			}
		}

		const auto a = mat[i][0];
		ret += sign * a * Det(sub_matrix);
		sign *= -1.0f;
	}

	return ret;
}

template <uint8 Row, uint8 Column>
Matrix<Row, Column> Matrix<Row, Column>::Invert(const Matrix<Row, Column>& mat) noexcept
{
	Matrix<Row, Column> ret = *this;
	ret.Inverted();
	return ret;
}

void Matrix<1, 1>::Inverted() noexcept
{
	mat[0][0] = 1.0f / mat[0][0];
}

void Matrix<2, 2>::Inverted() noexcept
{
	const Matrix<2, 2> ret{ mat[1][1], -mat[1][0], -mat[0][1], mat[0][0] };
	*this = ret / Det(*this);
}

void Matrix<3, 3>::Inverted() noexcept
{
	const Matrix<3, 3> ret
	{
		mat[1][1] * mat[2][2] - mat[1][2] * mat[2][1],
		mat[1][2] * mat[2][0] - mat[1][0] * mat[2][2],
		mat[1][0] * mat[2][1] - mat[1][1] * mat[2][0],
		mat[0][2] * mat[2][1] - mat[0][1] * mat[2][2],
		mat[0][0] * mat[2][2] - mat[0][2] * mat[2][0],
		mat[0][1] * mat[2][0] - mat[0][0] * mat[2][1],
		mat[0][1] * mat[1][2] - mat[0][2] * mat[1][1],
		mat[0][2] * mat[1][0] - mat[0][2] * mat[1][2],
		mat[0][0] * mat[1][1] - mat[0][1] * mat[1][0]
	};

	*this = ret / Det(*this);
}

template <uint8 Row, uint8 Column>
void Matrix<Row, Column>::Inverted() noexcept
{
	static_check(Row == Column);
	Matrix<Row, Column> ret;
	float sign = 1.0f;

	for (uint8 row = 0; row < Row; ++row)
	{
		for (uint8 column = 0; column < Column; ++column)
		{
			Matrix<Row - 1, Column - 1> sub_matrix;

			for (uint8 srcRow = 0, destRow = 0; srcRow < Row; ++srcRow)
			{
				if (srcRow == row) continue;
				for (uint8 srcColumn = 0, destColumn = 0; srcColumn < Column; ++srcColumn)
				{
					if (srcColumn == column) continue;

					sub_matrix[destRow][destColumn] = mat[srcRow][srcColumn];
					++destColumn;
				}

				++destRow;
			}

			ret(column, row) = sign * Det(sub_matrix);
			sign *= -1.0f;
		}
		sign *= -1.0f;
	}

	*this = ret / Det(*this);
}

template <uint8 Row, uint8 Column>
const char* Matrix<Row, Column>::ToString() const noexcept
{
	return "";
}

using Matrix2x2 = Matrix<2, 2>;
using Matrix3x3 = Matrix<3, 3>;
using Matrix4x4 = Matrix<4, 4>;