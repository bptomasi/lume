resource "aws_instance" "hello" {
  ami           = var.image_id
  instance_type = "t3.micro"
  tags = {
    Name = "hello"
  }
}
