#include "Mesh.h"
#include <fstream>
#include <map>
#include <sstream>
#include <rapidjson/document.h>
#include "JsonHelper.h"
#include "MathFunctions.h"
#include "ResourceManager.h"
#include "Texture.h"
#include "Vector3.h"
#include "VertexArray.h"

namespace ArenaBoss
{
	namespace
	{
		union Vertex
		{
			float f;
			uint8_t b[4];
		};
	}

	Mesh::Mesh(const std::string& inName, const std::string& fileName)
		: Resource(inName), textures(), vertexArray(nullptr), radius(0.0f), specPower(0.0f)
	{
		std::ifstream file{ "Asset\\" + fileName };
		if (!file.is_open())
			throw std::exception{ "Cannot found mesh file." };

		std::stringstream fileStream;
		fileStream << file.rdbuf();
		std::string contents = fileStream.str();
		rapidjson::StringStream jsonStr(contents.c_str());
		rapidjson::Document doc;
		doc.ParseStream(jsonStr);

		if (!doc.IsObject())
			throw std::exception{ "Mesh is not valid json" };

		auto& resourceManager = Accessor<ResourceManager>::GetManager();

		const auto& jsonTextures = doc["textures"];
		if (jsonTextures.IsArray())
		{
			for (rapidjson::SizeType i = 0; i < jsonTextures.Size(); ++i)
			{
				const std::string name = jsonTextures[i].GetString();
				auto texture = resourceManager.CreateResource<Texture>(name, name);

				if (texture == nullptr)
					texture = resourceManager.GetResource<Texture>("Default.png");

				textures.emplace_back(std::move(texture));
			}
		}

		const auto& jsonVerts = doc["vertices"];
		if (!jsonVerts.IsArray() || jsonVerts.Size() < 1)
			throw std::exception{ "Mesh has no vertices" };

		static std::map<std::string, std::pair<VertexLayout, size_t>> layoutMap
		{
			{ "PosNormTex", { VertexLayout::PosNormTex, 8 } },
			{ "PosNormSkinTex", { VertexLayout::PosNormSkinTex, 10 } }
		};

		auto [layout, vertSize] = layoutMap[doc["vertexformat"].GetString()];

		std::vector<Vertex> vertices;
		vertices.reserve(jsonVerts.Size() * vertSize);

		for (rapidjson::SizeType i = 0; i < jsonVerts.Size(); ++i)
		{
			const auto& vert = jsonVerts[i];
			if (!vert.IsArray())
				throw std::exception{ "Invalid vertex format" };

			const Math::Vector3 pos{ vert[0].GetFloat(), vert[1].GetFloat(), vert[2].GetFloat() };
			radius = Math::Max(radius, pos.LengthSqrt());

			switch (layout)
			{
			case VertexLayout::PosNormTex:
			{
				Vertex v;
				for (rapidjson::SizeType j = 0; j < vert.Size(); ++j)
				{
					v.f = vert[j].GetFloat();
					vertices.emplace_back(v);
				}
				break;
			}
			case VertexLayout::PosNormSkinTex:
				Vertex v;
				rapidjson::SizeType j;
				for (j = 0; j < 6; ++j)
				{
					v.f = vert[j].GetFloat();
					vertices.emplace_back(v);
				}
				for (j = 6; j < 14; j += 4)
				{
					v.b[0] = vert[j].GetUint();
					v.b[1] = vert[j + 1].GetUint();
					v.b[2] = vert[j + 2].GetUint();
					v.b[3] = vert[j + 3].GetUint();
					vertices.emplace_back(v);
				}
				for (j = 14; j < vert.Size(); ++j)
				{
					v.f = vert[j].GetFloat();
					vertices.emplace_back(v);
				}
				break;
			}
		}

		radius = Math::Sqrt(radius);

		const rapidjson::Value& jsonIndices = doc["indices"];
		if (!jsonIndices.IsArray() || jsonIndices.Size() < 1)
			throw std::exception{ "Mesh has no indices" };

		std::vector<uint> indices;
		indices.reserve(static_cast<size_t>(jsonIndices.Size()) * 3u);
		for (rapidjson::SizeType i = 0; i < jsonIndices.Size(); ++i)
		{
			const auto& index = jsonIndices[i];
			if (!index.IsArray() || index.Size() != 3)
				throw std::exception{ "Invalid indices" };

			indices.emplace_back(index[0].GetUint());
			indices.emplace_back(index[1].GetUint());
			indices.emplace_back(index[2].GetUint());
		}

		specPower = doc["specularPower"].GetFloat();

		VertexArrayParam param
		{
			layout,
			vertices.data(),
			static_cast<uint>(vertices.size() / vertSize),
			indices.data(),
			static_cast<uint>(indices.size())
		};

		vertexArray = resourceManager.CreateResource<VertexArray>("Vertex of " + GetName(), std::move(param));
	}

	Mesh::~Mesh()
	{
		Accessor<ResourceManager>::GetManager().DeleteResource("Vertex of " + GetName());
	}
}