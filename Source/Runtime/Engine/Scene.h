#pragma once

#include <string>
#include <vector>
#include "Accessor.h"

namespace ArenaBoss
{
	class Entity;

	class Scene final : public Accessor<class RenderManager>
	{
	public:
		Scene(const Scene&) = delete;
		Scene(Scene&&) = default;

		Scene& operator=(const Scene&) = delete;
		Scene& operator=(Scene&&) = default;

		~Scene() { Release(); }

		void Init();
		void Release() noexcept;

		void Load();
		void Load(const std::string& inName);
		void Load(std::string&& inName);

		void Save() const;
		void Save(const std::string& inName);
		void Save(std::string&& inName);

		Entity* AddEntity(const std::string& inName);
		Entity* AddEntity(Entity* entity);

		void RemoveEntity(const std::string& inName);
		void RemoveEntity(Entity* entity);

		Entity* GetEntity(const std::string& inName);

		inline const std::string& GetName() const noexcept { return name; }
		inline void SetName(const std::string& inName) noexcept { name = inName; }
		inline void SetName(std::string&& inName) noexcept { name = std::move(inName); }

	private:
		friend class SceneManager;
		Scene() : name(), entities() {}

		std::string name;
		std::vector<Entity*> entities;
	};
}