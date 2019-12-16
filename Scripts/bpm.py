# DownloadExternal.py
from threading import Thread
from pathlib import Path
from requests import get
import tarfile
import shutil
import os

def get_config_path():
    return Path.cwd().parent / 'Config' / 'ExternalPath.ini'

def get_cmake_path():
    return Path.cwd().parent / 'Cmake' / 'Package.cmake'

def make_abs_path(*names):
    ret = Path.cwd().parent / 'External'
    for name in names:
        ret /= name
    return ret

def init_folder():
    if os.path.exists(make_abs_path()):
        shutil.rmtree(make_abs_path())
    os.mkdir(make_abs_path())

def init_cmake():
    cmake_path = Path.cwd().parent / 'CMake' / 'Package.cmake'

    if cmake_path.exists():
        cmake_path.unlink()
    cmake_path.touch()

    cmake_in_path = Path.cwd().parent / 'CMake' / 'Package.cmake.in'
    cmake_path.write_text(cmake_in_path.read_text())

def add_package(name, url):
    print(f'Start downloading {name}.')
    path = make_abs_path(Path(url).name)
    path.write_bytes(get(url).content)

    print(f'Extracting {name}.')
    with tarfile.open(path) as file:
        file.extractall(make_abs_path())

    path.unlink()
    folder_path = Path(str(path)[:-7])
    folder_path.rename(make_abs_path(name))

def del_package(name):
    path = make_abs_path(name)
    shutil.rmtree(path)

def add_cmake(name):
    path = make_abs_path(name, 'CMakeLists.txt')
    command = ''
    
    if path.exists():
        command = 'add_subdirectory'
    else:
        path = path.parent
        for file in list(path.glob('**/*.cmake')):
            if path == f'Find{name}.cmake' or path == f'Find{name}.cmake':
                command = 'find_package'
                path = file
                break

    with get_cmake_path().open('a') as file:
        file.write(f'{command} ({path})\n')

def find_line(name, contents):
    contents = get_cmake_path().read_text().splitlines()
    for num, line in enumerate(contents):
        if name in line:
            return num
    raise ValueError

def del_cmake(name):
    contents = get_cmake_path().read_text().splitlines()
    try:
        line = find_line(name, contents)
        del contents[line:line + 1]
    except ValueError:
        print(f'{name} not found in cmake')

    with get_cmake_path().open() as file:
        for content in contents:
            file.write(content + '\n')

def get_config():
    config = {}
    for line in get_config_path().read_text().splitlines():
        name, url = line.split('=')
        config[name] = url
    return config

def set_config(config):
    with get_config_path().open('wt') as file:
        for pair in config.items():
            file.write(f'{pair[0]}={pair[1]}')

def init():
    init_folder()
    init_cmake()

def register(name, url):
    config = get_config()
    config[name] = url
    set_config(config)
    print(f'Successfully registered {name}')

def unregister(name):
    config = get_config()
    del config[name]
    set_config(config)
    print(f'Successfully unregistered {name}')

def search(name):
    return make_abs_path(name).exists()

def print_search(name):
    if (search(name)):
        print(f'{name} already installed.')
    else:
        print(f'{name} is not installed.')

def install_all():
    init()
    threads = []

    for name in get_config().keys():
        thread = Thread(target=install, args=(name))
        thread.start()
        threads.append(thread)

    for thread in threads:
        thread.join()

def install(name):
    if (name == 'all'):
        install_all()
        return

    if not search(name):
        add_package(name, get_config()[name])
        add_cmake(name)
        print(f'{name} has been added successfully.')
    else:
        print(f'{name} already installed.')

def uninstall(name):
    if search(name):
        del_package(name)
        del_cmake(name)
        print(f'Successfully deleted {name}.')
    else:
        print(f'{name} is not installed.')