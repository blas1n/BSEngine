#include "Texture.h"
#include <GL/glew.h>
#include <SDL2/SDL.h>
#include <SOIL2/SOIL2.h>

namespace ArenaBoss
{
	Texture::Texture(const std::string& inName, const std::string& fileName)
		: Resource(inName), textureId(0u), width(0), height(0)
	{
		auto channel = 0;
		const auto path = "Asset\\" + fileName;

		auto image = SOIL_load_image(path.c_str(),
			&width, &height, &channel, SOIL_LOAD_AUTO);

		if (image == nullptr)
			throw std::exception{ "SOIL failed to load image." };

		const auto format = channel == 4 ? GL_RGBA : GL_RGB;

		glGenTextures(1, &textureId);
		glBindTexture(GL_TEXTURE_2D, textureId);
		glTexImage2D(GL_TEXTURE_2D, 0, format, width, height, 0, format, GL_UNSIGNED_BYTE, image);
		SOIL_free_image_data(image);

		glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
		glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
	}

	Texture::~Texture()
	{
		glDeleteTextures(1, &textureId);
	}

	void Texture::Activate()
	{
		glBindTexture(GL_TEXTURE_2D, textureId);
	}
}