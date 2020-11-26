#pragma once

#include "Accessor.h"
#include "Resource.h"
#include <string>
#include <vector>

namespace ArenaBoss
{
	class Texture;
	class VertexArray;

	class Mesh final : public Resource, private Accessor<class ResourceManager>
	{
		GENERATE_RESOURCE(Mesh)

	public:
		Mesh(const std::string& inName, const std::string& fileName);

		Mesh(const Mesh&) = delete;
		Mesh(Mesh&&) = delete;

		Mesh& operator=(const Mesh&) = delete;
		Mesh& operator=(Mesh&&) = delete;

		~Mesh() override;

		inline const std::string& GetPath() const noexcept { return path; }

		inline Texture* GetTexture(size_t index) noexcept
		{
			return index < textures.size() ? textures[index] : nullptr;
		}

		inline const Texture* GetTexture(size_t index) const noexcept
		{
			return index < textures.size() ? textures[index] : nullptr;
		}

		inline VertexArray* GetVertexArray() noexcept { return vertexArray; }
		inline const VertexArray* GetVertexArray() const noexcept { return vertexArray; }

		inline float GetRadius() const noexcept { return radius; }
		inline float GetSpecularPower() const noexcept { return specPower; }

	private:
		std::string path;

		std::vector<Texture*> textures;
		VertexArray* vertexArray;
		float radius;
		float specPower;
	};
}