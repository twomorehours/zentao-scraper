import requests

def login(username, password):
    url = "http://192.168.58.94:3000/login"
    headers = {"Content-Type": "application/x-www-form-urlencoded"}
    data = {"username": username, "password": password}

    response = requests.post(url, headers=headers, data=data)

    if response.status_code == 200:
        json_data = response.json()
        token = json_data["token"]
        return token
    else:
        print("Login failed. Status code:", response.status_code)
        return None



token = login("yuhao", "1qaz@WSX")
if token:
    print("Token:", token)
else:
    print("Login failed.")