from multiprocessing import process
from combined_vision_tower import CombinedVisionTower
from transformers import CLIPImageProcessor
import torch

import onnx
from onnx.onnx_pb import ModelProto
from onnxruntime.quantization import QuantizationMode
from onnxruntime.quantization.onnx_quantizer import ONNXQuantizer
from onnxruntime.quantization.registry import IntegerOpsRegistry

import requests
from PIL import Image
from io import BytesIO

model_name = "openai/clip-vit-large-patch14"
image_preprocessor = CLIPImageProcessor.from_pretrained(model_name)
model = CombinedVisionTower(model_name)

def load_image(image_file):
    if image_file.startswith('http') or image_file.startswith('https'):
        response = requests.get(image_file)
        image = Image.open(BytesIO(response.content)).convert('RGB')
    else:
        image = Image.open(image_file).convert('RGB')
    return image

def prepare_onnx(samples, model_name="model_prequantized", log=False):
    preprocessed_image = image_preprocessor.preprocess(samples, return_tensors='pt')['pixel_values'].half().cuda()

    features = model(preprocessed_image)

    if log:
        print("Input tensor size, type:", preprocessed_image.shape, preprocessed_image.dtype)
        print("Output tensor size, type:", features.shape, features.dtype)

    filepath = f"{model_name}.onnx"
    # Saving our weights
    torch.onnx.export(
        model, 
        (preprocessed_image),
        filepath,
        input_names=["preprocessed_image"],
        output_names=["embeddings"],
        dynamic_axes={
            "preprocessed_image": [0],
            "embeddings": [0],
        }
    )

    return model_name

def quantize(model_name="model", original_model="model_prequantized", log=False):
    onnx_model = onnx.load(f"{original_model}.onnx")

    copy_model = ModelProto()
    copy_model.CopyFrom(onnx_model)

    # Needs onnxruntime==1.13.1
    quantizer = ONNXQuantizer(
        model=copy_model,
        per_channel=False,
        reduce_range=False,
        mode=QuantizationMode.IntegerOps,
        static=False,
        weight_qType=True,
        activation_qType=False,
        tensors_range=None,
        nodes_to_quantize=None,
        nodes_to_exclude=None,
        op_types_to_quantize=list(IntegerOpsRegistry),
    )

    quantizer.quantize_model()

    # Append "-quantized" at the end of the model's name
    quantized_model_path = f"{model_name}.onnx"

    # Save model
    if log:
        print(f"Quantized model has been written at {quantized_model_path}: \N{heavy check mark}")
    onnx.save_model(quantizer.model.model, quantized_model_path)

# Pipeline
sample = load_image("https://thumbs.dreamstime.com/b/golden-retriever-dog-21668976.jpg")

original_model = prepare_onnx(sample, log=True)
quantize(original_model=original_model, log=True)
