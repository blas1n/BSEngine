#pragma once

#include "Resource.h"
#include <string>

namespace ArenaBoss
{
	class Texture : public Resource
	{
		GENERATE_RESOURCE(Texture)

	public:
		Texture(const std::string& inName, const std::string& fileName);
		~Texture();

		void Activate();

		inline int GetWidth() const noexcept { return width; }
		inline int GetHeight() const noexcept { return height; }

	private:
		unsigned int textureId;

		int width;
		int height;
	};
}