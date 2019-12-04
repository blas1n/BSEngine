#include "Matrix3x3.h"
#include "Vector2.h"

namespace BE::Math
{
	Matrix3x3 Matrix3x3::FromScale(const Vector2& scale)
	{
		Matrix3x3 ret;
		ret.mat = Eigen::Scaling(scale[0], scale[1]).toDenseMatrix();
		return ret;
	}

	Matrix3x3 Matrix3x3::FromPosition(const Vector2& pos)
	{
		Eigen::Affine2f transform{ Eigen::Translation2f{ pos[0], pos[1] } };
		
		Matrix3x3 ret;
		ret.mat = transform.matrix();
		return ret;
	}

	Matrix3x3 Matrix3x3::FromRotation(const float angle)
	{
		Eigen::Affine2f transform{ Eigen::Rotation2Df{ angle } };

		Matrix3x3 ret;
		ret.mat = transform.matrix();
		return ret;
	}
}