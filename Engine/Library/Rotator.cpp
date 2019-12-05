#include "Rotator.h"
#include <Eigen/Geometry>
#include "MathFunctions.h"
#include "Matrix4x4.h"
#include "Quaternion.h"

namespace BE::Math
{
	Rotator& Rotator::operator+=(const Rotator& other) noexcept
	{
		euler += other.euler;
		
		for (int i = 0; i < 3; ++i)
			euler[i] = Fmod(euler[i], 360.0f);
	}

	Rotator& Rotator::operator-=(const Rotator& other) noexcept
	{
		euler -= other.euler;

		for (int i = 0; i < 3; ++i)
			while (euler[i] < 0.0f)
				euler[i] += 360.0f;
	}

	Rotator& Rotator::operator*=(const Vector3& scale) noexcept
	{
		euler *= scale;

		for (int i = 0; i < 3; ++i)
			euler[i] = Fmod(euler[i], 360.0f);
	}

	Rotator& Rotator::operator/=(const Vector3& scale) noexcept
	{
		euler /= scale;
	}

	Rotator& Rotator::operator*=(const float scale) noexcept
	{
		euler *= scale;

		for (int i = 0; i < 3; ++i)
			euler[i] = Fmod(euler[i], 360.0f);
	}

	Rotator& Rotator::operator/=(const float scale) noexcept
	{
		euler /= scale;
	}

	Matrix4x4 Rotator::ToMatrix() const noexcept
	{
		const Eigen::Matrix3f mat
		{
			Eigen::AngleAxisf{ ToRadians(roll()), Eigen::Vector3f::UnitX() } *
				Eigen::AngleAxisf{ ToRadians(pitch()), Eigen::Vector3f::UnitY() } *
				Eigen::AngleAxisf{ ToRadians(yaw()), Eigen::Vector3f::UnitZ() }
		};

		return Matrix4x4{ mat.data() };
	}

	Quaternion Rotator::ToQuaternion() const noexcept
	{
		const Eigen::Quaternionf quat
		{
			Eigen::AngleAxisf{ ToRadians(roll()), Eigen::Vector3f::UnitX() } *
			Eigen::AngleAxisf{ ToRadians(pitch()), Eigen::Vector3f::UnitY() } *
			Eigen::AngleAxisf{ ToRadians(yaw()), Eigen::Vector3f::UnitZ() }
		};

		return Quaternion{ quat.x(), quat.y(), quat.z(), quat.w() };
	}
}