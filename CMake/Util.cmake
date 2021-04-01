function (register_module)
    get_filename_component (module_name ${cmake_current_source_dir} name)
    
    file (GLOB_RECURSE srcs "*.cpp")
    add_library (${module_name} SHARED ${srcs})
    target_include_directories (${module_name} PUBLIC "Public")

    string (TOUPPER ${module_name}_API api)
    target_compile_definitions (${module_name} PRIVATE ${api}=${dll_export} INTERFACE ${api}=${dll_import})

    if (EXISTS "${cmake_current_source_dir}/Public/pch.h")
        target_precompile_headers (${module_name} PUBLIC Public/pch.h)
    endif ()

    if (EXISTS "${cmake_current_source_dir}/Private/pch.h")
        target_precompile_headers (${module_name} PRIVATE Private/pch.h)
    endif ()
endfunction ()

function (download_package name REPO TAG)
    set (cmake_dir "${project_source_dir}/CMake/AutoInstall")
    set (src_dir "${project_source_dir}/Source/ThirdParty/${name}")
    set (build_dir "${cmake_binary_dir}/ThirdParty/${name}-build")
    set (download_dir "${cmake_binary_dir}/ThirdParty/${name}-download")
    configure_file ("${cmake_dir}/${name}.cmake" "${download_dir}/CMakeLists.txt" @ONLY)

    message ("Dir: ${src_dir}, ${build_dir} ${download_dir}")

    execute_process (
        COMMAND ${cmake_command} -G "${cmake_generator}" .
        RESULT_VARIABLE result
        WORKING_DIRECTORY "${download_dir}"
    )

    if (result)
        message (FATAL_ERROR "CMake step for ${name} failed: ${result}")
    endif ()

    execute_process (
        COMMAND ${cmake_command} --build .
        RESULT_VARIABLE result
        WORKING_DIRECTORY "${download_dir}"
    )

    if (result)
        message (FATAL_ERROR "Build step for ${name} failed: ${result}")
    endif ()

    if (EXISTS "${cmake_dir}/${name}-build.cmake")
         configure_file ("${cmake_dir}/${name}-build.cmake" "${src_dir}/CMakeLists.txt" @ONLY)
    endif ()

    if (EXISTS "${src_dir}/CMakeLists.txt")
         add_subdirectory ("${src_dir}" "${build_dir}" EXCLUDE_FROM_ALL)
    endif ()
endfunction ()

function (get_package name)
    if (${argc} GREATER 1)
         set(version ${argv1})
    endif ()

    if (version)
        find_package (${name} ${version} QUIET)
    else ()
        find_package (${name} QUIET)
    endif ()
    
    if (${name}_FOUND)
        message (STATUS "Found ${name} from system")
    else ()
        message (STATUS "Could not find ${name} from system. Downloading...")
        download_package (${name})
    endif ()
endfunction ()
